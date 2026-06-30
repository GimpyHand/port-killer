use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use gtk4::gdk::Key;
use gtk4::glib;
use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CssProvider, Label, ListBox, ListBoxRow, Orientation,
    ScrolledWindow, STYLE_PROVIDER_PRIORITY_APPLICATION,
};
use libadwaita::prelude::*;
use libadwaita::{HeaderBar, MessageDialog, ResponseAppearance, ToolbarView, Window};

use crate::group::{build_kill_targets, KillTarget};
use crate::kill::{kill_pids, refresh_waybar};
use crate::listener::collect_listeners;

const WAYBAR_SIGNAL: u8 = 9;
const GUI_ENV: &str = "PORT_KILLER_GUI";
const GUI_MODE_ENV: &str = "PORT_KILLER_GUI_MODE";

pub fn gui_env_var() -> &'static str {
    GUI_ENV
}

pub fn run_dropdown() -> Result<(), String> {
    run_gui("dropdown")
}

pub fn run_window() -> Result<(), String> {
    run_gui("window")
}

fn run_gui(mode: &str) -> Result<(), String> {
    if std::env::args_os().len() > 1 {
        return respawn_gui_process(mode);
    }

    if std::env::var_os(GUI_ENV).is_none() {
        return Err("internal GUI launcher error".to_string());
    }

    gtk4::init().map_err(|e| format!("failed to start GTK: {e}"))?;
    libadwaita::init().map_err(|e| format!("failed to start libadwaita: {e}"))?;
    install_label_measure_fix();

    let listeners = collect_listeners()?;
    if listeners.is_empty() {
        return Ok(());
    }

    let targets = build_kill_targets(listeners);
    let error: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let main_loop = Rc::new(glib::MainLoop::new(None, false));

    if mode == "window" {
        open_window(&targets, error.clone(), main_loop.clone())?;
    } else {
        open_dropdown(&targets, error.clone(), main_loop.clone())?;
    }

    main_loop.run();

    let outcome = match error.borrow_mut().take() {
        Some(message) => Err(message),
        None => Ok(()),
    };
    outcome
}

fn open_dropdown(
    targets: &[KillTarget],
    error: Rc<RefCell<Option<String>>>,
    main_loop: Rc<glib::MainLoop>,
) -> Result<(), String> {
    let height = (targets.len() as i32 * 64).clamp(180, 420);
    let window = Window::builder()
        .title("Port Killer")
        .default_width(420)
        .default_height(height)
        .resizable(false)
        .build();

    {
        let main_loop = main_loop.clone();
        window.connect_close_request(move |_| {
            main_loop.quit();
            glib::Propagation::Proceed
        });
    }
    bind_escape_quit(&window, main_loop.clone());

    let list = ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.add_css_class("boxed-list");
    list.add_css_class("port-killer-list");

    for target in targets {
        let row = build_target_row(target, true);
        row.set_activatable(true);

        let target_kill = target.clone();
        let error_kill = error.clone();
        let main_loop_kill = main_loop.clone();
        let window_ref = window.clone();

        row.connect_activate(move |_| {
            let dialog = MessageDialog::builder()
                .heading("Kill server?")
                .body(format!("Stop {}?", target_kill.panel_label()))
                .transient_for(&window_ref)
                .modal(true)
                .build();
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("kill", "Kill");
            dialog.set_response_appearance("kill", ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");

            let target_dialog = target_kill.clone();
            let error_dialog = error_kill.clone();
            let main_loop_dialog = main_loop_kill.clone();
            dialog.connect_response(Some("kill"), move |_, _| {
                match kill_targets(&[target_dialog.clone()]) {
                    Ok(_) => {
                        refresh_waybar(WAYBAR_SIGNAL);
                        main_loop_dialog.quit();
                    }
                    Err(message) => {
                        *error_dialog.borrow_mut() = Some(message);
                        main_loop_dialog.quit();
                    }
                }
            });
            dialog.present();
        });

        list.append(&row);
    }

    let scroll = ScrolledWindow::builder()
        .vexpand(true)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();
    scroll.set_child(Some(&list));
    window.set_content(Some(&scroll));
    window.present();
    Ok(())
}

fn open_window(
    targets: &[KillTarget],
    error: Rc<RefCell<Option<String>>>,
    main_loop: Rc<glib::MainLoop>,
) -> Result<(), String> {
    let window = Window::builder()
        .title("Port Killer")
        .default_width(640)
        .default_height(480)
        .build();

    {
        let main_loop = main_loop.clone();
        window.connect_close_request(move |_| {
            main_loop.quit();
            glib::Propagation::Proceed
        });
    }
    bind_escape_quit(&window, main_loop.clone());

    let header = HeaderBar::new();
    let title = Label::new(Some("Dev servers"));
    title.add_css_class("title");
    header.set_title_widget(Some(&title));

    let toolbar = ToolbarView::new();
    toolbar.add_top_bar(&header);

    let root = GtkBox::new(Orientation::Vertical, 0);
    let list = ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.add_css_class("boxed-list");
    list.add_css_class("port-killer-list");

    let rows: Rc<RefCell<Vec<(gtk4::CheckButton, KillTarget)>>> =
        Rc::new(RefCell::new(Vec::new()));

    for target in targets {
        let check = gtk4::CheckButton::new();
        let row = build_target_row(target, true);
        row.set_activatable(false);
        row.set_selectable(false);

        if let Some(container) = row.child().and_downcast::<GtkBox>() {
            container.prepend(&check);
        }

        list.append(&row);
        rows.borrow_mut().push((check, target.clone()));
    }

    let scroll = ScrolledWindow::builder()
        .vexpand(true)
        .margin_start(12)
        .margin_end(12)
        .margin_top(8)
        .margin_bottom(8)
        .build();
    scroll.set_child(Some(&list));

    let actions = GtkBox::new(Orientation::Horizontal, 12);
    actions.set_margin_start(12);
    actions.set_margin_end(12);
    actions.set_margin_bottom(12);
    actions.set_halign(Align::End);

    let cancel = Button::with_label("Cancel");
    let kill = Button::with_label("Kill selected");
    kill.add_css_class("destructive-action");

    {
        let main_loop = main_loop.clone();
        cancel.connect_clicked(move |_| main_loop.quit());
    }

    {
        let rows = rows.clone();
        let window = window.clone();
        let error = error.clone();
        let main_loop = main_loop.clone();
        kill.connect_clicked(move |_| {
            let selected: Vec<KillTarget> = rows
                .borrow()
                .iter()
                .filter(|(check, _)| check.is_active())
                .map(|(_, target)| target.clone())
                .collect();

            if selected.is_empty() {
                return;
            }

            let process_count: usize = selected.iter().map(|t| t.all_pids().len()).sum();
            let dialog = MessageDialog::builder()
                .heading("Kill servers?")
                .body(format!(
                    "Stop {} group(s) ({} process(es) total)?",
                    selected.len(),
                    process_count
                ))
                .transient_for(&window)
                .modal(true)
                .build();
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("kill", "Kill");
            dialog.set_response_appearance("kill", ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");

            let error_kill = error.clone();
            let selected_kill = selected.clone();
            let main_loop_kill = main_loop.clone();
            dialog.connect_response(Some("kill"), move |_, _| {
                match kill_targets(&selected_kill) {
                    Ok(_) => {
                        refresh_waybar(WAYBAR_SIGNAL);
                        main_loop_kill.quit();
                    }
                    Err(message) => {
                        *error_kill.borrow_mut() = Some(message);
                        main_loop_kill.quit();
                    }
                }
            });
            dialog.present();
        });
    }

    root.append(&scroll);
    root.append(&actions);
    toolbar.set_content(Some(&root));
    window.set_content(Some(&toolbar));
    window.present();
    Ok(())
}

fn build_target_row(target: &KillTarget, boxed: bool) -> ListBoxRow {
    let row = ListBoxRow::new();
    if boxed {
        row.add_css_class("port-killer-row");
    }

    let outer = GtkBox::new(Orientation::Horizontal, 12);
    outer.set_margin_top(10);
    outer.set_margin_bottom(10);
    outer.set_margin_start(12);
    outer.set_margin_end(12);

    let labels = GtkBox::new(Orientation::Vertical, 2);
    labels.set_hexpand(true);
    labels.append(&make_row_label(&target.panel_label(), true));

    let detail = target.panel_detail();
    if !detail.trim().is_empty() {
        labels.append(&make_row_label(&detail, false));
    }

    outer.append(&labels);
    row.set_child(Some(&outer));
    row
}

fn make_row_label(text: &str, heading: bool) -> Label {
    let label = Label::builder()
        .label(text)
        .halign(Align::Start)
        .xalign(0.0)
        .hexpand(true)
        .wrap(false)
        .ellipsize(EllipsizeMode::End)
        .build();

    if heading {
        label.add_css_class("heading");
    } else {
        label.add_css_class("dim-label");
    }

    label
}

fn install_label_measure_fix() {
    let provider = CssProvider::new();
    provider.load_from_data(
        "list.port-killer-list label { min-width: 0; }
         .port-killer-row label { min-width: 0; }",
    );

    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn bind_escape_quit(window: &Window, main_loop: Rc<glib::MainLoop>) {
    let controller = gtk4::EventControllerKey::new();
    controller.connect_key_released(move |_, key, _, _| {
        if key == Key::Escape {
            main_loop.quit();
        }
    });
    window.add_controller(controller);
}

fn kill_targets(targets: &[KillTarget]) -> Result<u32, String> {
    let pids: HashSet<u32> = targets.iter().flat_map(KillTarget::all_pids).collect();
    kill_pids(&pids, false).map_err(|e| e.to_string())
}

fn respawn_gui_process(mode: &str) -> Result<(), String> {
    use std::process::Command;

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let status = Command::new(&exe)
        .env(GUI_ENV, "1")
        .env(GUI_MODE_ENV, mode)
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "graphical menu exited with status {}",
            status.code().unwrap_or(-1)
        ))
    }
}
