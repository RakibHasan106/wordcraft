//! Keyboard: typing, editing keys and command shortcuts (the user's custom keys from Customize
//! Keyboard first, then the registry's `shortcut` fields).

use egui::{Key, Modifiers};
use serde_json::{Value, json};

use crate::WordApp;
use wordcraft_engine::ImeEditOutcome;

/// The registry's name for a key.
pub fn key_name(k: Key) -> Option<&'static str> {
    Some(match k {
        Key::ArrowLeft => "Left",
        Key::ArrowRight => "Right",
        Key::ArrowUp => "Up",
        Key::ArrowDown => "Down",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::Enter => "Enter",
        Key::Tab => "Tab",
        Key::Backspace => "Backspace",
        Key::Delete => "Delete",
        Key::Escape => "Escape",
        Key::Space => "Space",
        Key::Insert => "Insert",
        Key::Equals | Key::Plus => "=",
        Key::Minus => "-",
        Key::Period => ".",
        Key::Comma => ",",
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::Num0 => "0",
        Key::Num1 => "1",
        Key::Num2 => "2",
        Key::Num3 => "3",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        Key::Num7 => "7",
        Key::Num8 => "8",
        Key::Num9 => "9",
        Key::F1 => "F1",
        Key::F2 => "F2",
        Key::F3 => "F3",
        Key::F4 => "F4",
        Key::F5 => "F5",
        Key::F6 => "F6",
        Key::F7 => "F7",
        Key::F8 => "F8",
        Key::F9 => "F9",
        Key::F10 => "F10",
        Key::F11 => "F11",
        Key::F12 => "F12",
        Key::A => "A",
        Key::B => "B",
        Key::C => "C",
        Key::D => "D",
        Key::E => "E",
        Key::F => "F",
        Key::G => "G",
        Key::H => "H",
        Key::I => "I",
        Key::J => "J",
        Key::K => "K",
        Key::L => "L",
        Key::M => "M",
        Key::N => "N",
        Key::O => "O",
        Key::P => "P",
        Key::Q => "Q",
        Key::R => "R",
        Key::S => "S",
        Key::T => "T",
        Key::U => "U",
        Key::V => "V",
        Key::W => "W",
        Key::X => "X",
        Key::Y => "Y",
        Key::Z => "Z",
        _ => return None,
    })
}

/// A key with its modifiers as the registry writes shortcuts (`Mod+Shift+K`).
pub fn combo(m: Modifiers, key: &str, with_shift: bool) -> String {
    let mut s = String::new();
    if m.command {
        s.push_str("Mod+");
    }
    if m.ctrl && !m.command {
        s.push_str("Ctrl+");
    }
    if m.mac_cmd && m.ctrl {
        s.push_str("Ctrl+");
    }
    if m.alt {
        s.push_str("Alt+");
    }
    if m.shift && with_shift {
        s.push_str("Shift+");
    }
    s.push_str(key);
    s
}

/// Shortcuts for interface commands (`ui.*`), which aren't in the engine's keymap: Alt+Q moves
/// to "Tell me" command search, as in Word for Windows (#819). Not on macOS, where Option+Q types
/// a character. On macOS ⌘⌥R collapses or expands the ribbon, as in Word for Mac.
fn ui_shortcut(key: Key, m: Modifiers) -> Option<(&'static str, Value)> {
    let alt_only = m.alt && !m.command && !m.ctrl && !m.mac_cmd && !m.shift;
    let mac_cmd_alt = cfg!(target_os = "macos") && m.mac_cmd && m.alt && !m.ctrl && !m.shift;
    match key {
        Key::Q if alt_only && !cfg!(target_os = "macos") => Some(("ui.dialog", json!({"name": "commands"}))),
        Key::R if mac_cmd_alt => Some(("ui.collapseRibbon", json!({}))),
        // Ctrl+F1 collapses or expands the ribbon, as in Word for Windows (#1678). Not on macOS,
        // where Ctrl+F1 is the system's keyboard-access switch.
        Key::F1 if m.ctrl && !m.alt && !m.shift && !m.mac_cmd && !cfg!(target_os = "macos") => Some(("ui.collapseRibbon", json!({}))),
        _ => None,
    }
}

/// Find and run the command bound to this key. Returns true if handled.
fn dispatch(app: &mut WordApp, key: Key, m: Modifiers) -> bool {
    let reg = app.session.registry.clone();
    let name = key_name(key);
    let bound = name.and_then(|n| app.session.keymap.resolve(&reg, &combo(m, n, true)));
    // Ctrl+F1 collapses the ribbon only when the user hasn't given it to a command (#1678); the
    // other interface shortcuts (Alt+Q, ⌘⌥R) keep their place ahead of the keymap.
    if let Some((id, params)) = ui_shortcut(key, m).filter(|_| key != Key::F1 || bound.is_none()) {
        let _ = app.run(id, params);
        return true;
    }
    if let Some(spec) = bound {
        // Option+Shift+- types an em dash on macOS; Word's Alt+Shift+- (Collapse) only works in
        // Outline view, so elsewhere the key stays typing (#1933).
        if m.alt && !crate::widgets::enabled(app, spec.id) {
            return false;
        }
        let _ = app.run(spec.id, json!({}));
        return true;
    }
    let Some(name) = name else { return false };
    // Shift extends caret movement; Shift+Backspace still erases (#707).
    if m.shift
        && let Some(spec) = app.session.keymap.resolve(&reg, &combo(m, name, false))
        && let Some(params) = shift_variant_params(spec.id)
    {
        let _ = app.run(spec.id, params);
        return true;
    }
    false
}

/// How a command bound to a plain key runs when Shift is also held and nothing is bound to the
/// shifted combo: caret moves extend the selection, Backspace erases as usual (#707).
fn shift_variant_params(id: &str) -> Option<Value> {
    if id.starts_with("caret.") {
        Some(json!({"extend": true}))
    } else if id == "text.backspace" {
        Some(json!({}))
    } else {
        None
    }
}

/// Shift+Delete, which cuts the selection and does nothing without one, as in Word for Windows
/// (#1669). On Windows the windowing layer already sends it as a Cut; elsewhere it arrives as the
/// key, and no shortcut was bound to it.
fn is_shift_delete(key: Key, m: Modifiers) -> bool {
    key == Key::Delete && m.shift && !m.command && !m.ctrl && !m.alt && !m.mac_cmd
}

/// Shift+F10 opens the context menu, as the Menu key does (#821). (The Menu key itself doesn't
/// reach the app: the windowing layer drops it.)
fn is_context_menu_key(key: Key, m: Modifiers) -> bool {
    key == Key::F10 && m.shift && !m.command && !m.alt && !m.ctrl
}

/// F1–F12: shortcuts on these work without the canvas focused.
fn is_function_key(key: Key) -> bool {
    matches!(key, Key::F1 | Key::F2 | Key::F3 | Key::F4 | Key::F5 | Key::F6 | Key::F7 | Key::F8 | Key::F9 | Key::F10 | Key::F11 | Key::F12)
}

/// Keys while editing inside an equation. Returns true when handled.
fn equation_key(app: &mut WordApp, key: Key, m: Modifiers) -> bool {
    if m.command || m.alt || (m.ctrl && !cfg!(target_os = "macos")) {
        return false;
    }
    let (id, params) = match key {
        Key::ArrowLeft => ("equation.move", json!({"dir": "left"})),
        Key::ArrowRight => ("equation.move", json!({"dir": "right"})),
        Key::ArrowUp => ("equation.move", json!({"dir": "up"})),
        Key::ArrowDown => ("equation.move", json!({"dir": "down"})),
        Key::Home => ("equation.move", json!({"dir": "home"})),
        Key::End => ("equation.move", json!({"dir": "end"})),
        Key::Tab => ("equation.move", json!({"dir": if m.shift { "prev" } else { "next" }})),
        Key::Backspace => ("equation.backspace", json!({})),
        Key::Delete => ("equation.delete", json!({})),
        Key::Enter => ("equation.enter", json!({})),
        Key::Escape => ("equation.exit", json!({})),
        _ => return false,
    };
    let _ = app.run(id, params);
    true
}

/// Whether releasing `key` with `modifiers` is the plain Paste shortcut (Mod+V or the Paste
/// key) — not Mod+Shift+V (Paste Formatting) or Mod+Alt+V (Paste Special) — so a clipboard
/// picture may be pasted on it (#45).
fn is_paste_release(key: Key, modifiers: egui::Modifiers) -> bool {
    (modifiers.command || key == Key::Paste) && !modifiers.alt && !modifiers.shift
}

/// The modifiers a V release stands for: its own, or, when Mod went up before V and no V press
/// came (egui-winit keeps Mod+V presses), those held with Mod.
fn paste_release_modifiers(released: Modifiers, press_seen: bool, held: Option<Modifiers>) -> Modifiers {
    if released.command || press_seen { released } else { held.unwrap_or(released) }
}

/// Whether `key` is a Shift, Ctrl, Alt or Super key, whose own presses don't end a Mod+V.
fn is_modifier_key(key: Key) -> bool {
    matches!(
        key,
        Key::ShiftLeft | Key::ShiftRight | Key::ControlLeft | Key::ControlRight | Key::AltLeft | Key::AltRight | Key::SuperLeft | Key::SuperRight
    )
}

/// Whether releasing `key` with `modifiers` is Mod+Alt+V (Paste Special).
fn is_paste_special_release(key: Key, modifiers: egui::Modifiers) -> bool {
    key == Key::V && modifiers.command && modifiers.alt && !modifiers.shift
}

/// Events for the focused canvas: text, editing keys, clipboard, IME. While keytips are showing
/// every key belongs to them (letters pick badges, Escape cancels), so nothing here runs.
pub fn canvas_events(app: &mut WordApp, ctx: &egui::Context) {
    if app.ui.keytips != crate::keytips::Phase::Off {
        return;
    }
    let events = ctx.input(|i| i.events.clone());
    if app.session.view.read_mode {
        // Read Mode is for navigation and selection, not typing into the document. The ribbon's
        // Edit Document button or Escape returns to Print Layout before editing.
        for e in &events {
            match e {
                // Not while a dialog or menu that takes Escape is open.
                egui::Event::Key { key: Key::Escape, pressed: true, .. } if app.escape_free => {
                    // Focus or Full Screen first (#1678), then Read Mode.
                    let id = crate::chrome::Screen::of(&app.session.view).exit_command().unwrap_or("view.printLayout");
                    let _ = app.run(id, json!({"value": false}));
                    break;
                }
                egui::Event::Key { key: Key::PageDown | Key::ArrowRight, pressed: true, modifiers, .. } if modifiers.is_none() => {
                    crate::canvas::turn_read_page(app, 1.0)
                }
                egui::Event::Key { key: Key::PageUp | Key::ArrowLeft, pressed: true, modifiers, .. } if modifiers.is_none() => {
                    crate::canvas::turn_read_page(app, -1.0)
                }
                egui::Event::Copy => {
                    let selected = app.session.selected_text();
                    if !selected.is_empty() {
                        ctx.copy_text(selected);
                    }
                }
                _ => {}
            }
        }
        return;
    }
    let alt_codes = alt_codes_supported(ctx);
    // `DeleteSurrounding` and the text replacing what it erased arrive in the same frame.
    app.canvas.ime_join_text = None;
    // Mod held since an earlier frame (its ModifiersChanged went by then).
    let input_mods = ctx.input(|i| i.modifiers);
    let held = if input_mods.is_none() { app.canvas.modifiers } else { input_mods };
    if held.command {
        app.canvas.paste_mods = Some(held);
    }
    // Super (Win / Logo / Cmd) held since an earlier frame, updated in event order below.
    let mut super_key_held = app.canvas.super_held;
    let mut mac_cmd_held = ctx.input(|i| i.modifiers.mac_cmd);
    let mut current_mods = held;
    for e in events {
        // Track Super and Command state in event order so same-frame shortcuts are detected.
        match &e {
            egui::Event::Key { key: Key::SuperLeft | Key::SuperRight, pressed, modifiers, .. } => {
                super_key_held = *pressed;
                mac_cmd_held = modifiers.mac_cmd;
                current_mods = *modifiers;
            }
            egui::Event::Key { modifiers, .. } => {
                current_mods = *modifiers;
            }
            egui::Event::ModifiersChanged(m) => {
                mac_cmd_held = m.mac_cmd;
                let had_other_mods = current_mods.shift || current_mods.ctrl || current_mods.alt;
                current_mods = *m;
                if m.is_none() && !had_other_mods {
                    super_key_held = false;
                }
            }
            egui::Event::WindowFocused(false) => {
                super_key_held = false;
                mac_cmd_held = false;
                current_mods = egui::Modifiers::default();
            }
            _ => {}
        }
        let super_held = super_key_held || mac_cmd_held;

        // Which presses reached the canvas, and the modifiers held with Mod, in event order: Mod
        // can go down and up within one frame, leaving the frame's modifiers without it.
        match &e {
            egui::Event::ModifiersChanged(m) if m.command => app.canvas.paste_mods = Some(*m),
            egui::Event::Key { key: Key::V | Key::Paste, pressed: true, .. } => app.canvas.v_pressed = true,
            egui::Event::Key { key, pressed: true, modifiers, .. } if !is_modifier_key(*key) => {
                app.canvas.paste_mods = modifiers.command.then_some(*modifiers);
            }
            egui::Event::WindowFocused(_) => {
                app.canvas.v_pressed = false;
                app.canvas.paste_mods = None;
            }
            _ => {}
        }
        // An Alt shortcut (Alt+Shift+D, Alt+F7…) may also send its key's text; it isn't typed.
        match &e {
            egui::Event::Key { pressed: true, .. } => {
                app.canvas.swallow_text = false;
                app.canvas.alt_code_echo = None;
                app.canvas.ime_join_text = None;
            }
            egui::Event::Text(_) if std::mem::take(&mut app.canvas.swallow_text) => continue,
            egui::Event::Text(t) => {
                if let Some(echo) = app.canvas.alt_code_echo.take()
                    && alt_code_echoed(app, &echo, t)
                {
                    continue;
                }
            }
            // Focus elsewhere (another window) ends an Alt code unfinished.
            egui::Event::WindowFocused(false) => app.canvas.alt_code.clear(),
            _ => {}
        }
        if alt_codes && app.session.math.is_none() && alt_code_event(app, &e) {
            continue;
        }
        if let egui::Event::Paste(t) = &e {
            app.canvas.pasted = true;
            // Copied picture files arrive as their paths: insert the pictures (#45).
            if app.session.math.is_none() && !ctx.input(|i| i.modifiers.alt) && crate::paste_picture::paste_named_files(app, t) {
                continue;
            }
        }
        // Desktop: Mod+V with no text on the clipboard sends no paste event, only the key's
        // release; paste the clipboard's picture then (#45). A paste event before the release
        // means it was text. (The web reads pasted pictures itself.) Mod released before V still
        // pastes: a release with no V press seen was Mod+V.
        if let egui::Event::Key { key: key @ (Key::V | Key::Paste), pressed: false, modifiers, .. } = &e {
            let press_seen = std::mem::take(&mut app.canvas.v_pressed);
            let modifiers = &paste_release_modifiers(*modifiers, press_seen, app.canvas.paste_mods.take());
            let paste_keys = is_paste_release(*key, *modifiers);
            let pasted = std::mem::take(&mut app.canvas.pasted);
            if !pasted && paste_keys && app.session.math.is_none() && app.services.clipboard_picture.is_some() {
                let _ = app.run("edit.paste", json!({}));
            }
            // Mod+Alt+V with no text on the clipboard: Paste Special still opens, listing what
            // is there (a picture) or saying what it holds (#687, #833).
            if !pasted && is_paste_special_release(*key, *modifiers) && app.session.math.is_none() && app.services.clipboard_picture.is_some() {
                let _ = app.run("edit.pasteSpecial", json!({}));
            }
            continue;
        }
        // Editing an equation: text and editing keys go into it.
        if app.session.math.is_some() {
            match &e {
                egui::Event::Text(t) => {
                    let m = ctx.input(|i| i.modifiers);
                    let is_cmd = if cfg!(target_os = "macos") { m.command } else { (m.command || m.ctrl) && !m.alt };
                    if !is_cmd && !super_held && !t.is_empty() && t.chars().all(|c| !c.is_control()) {
                        let _ = app.run("equation.type", json!({"text": t}));
                    }
                    continue;
                }
                egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                    app.canvas.ime_preedit.clear();
                    app.canvas.ime_preedit_range = None;
                    if !text.is_empty() {
                        let _ = app.run("equation.type", json!({"text": text}));
                    }
                    continue;
                }
                egui::Event::Paste(t) => {
                    let _ = app.run("equation.type", json!({"text": t}));
                    continue;
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } if equation_key(app, *key, *modifiers) => continue,
                _ => {}
            }
        }
        match e {
            egui::Event::Text(t) => {
                let m = ctx.input(|i| i.modifiers);
                let is_cmd = if cfg!(target_os = "macos") { m.command } else { (m.command || m.ctrl) && !m.alt };
                if is_cmd || super_held {
                    continue;
                }
                if t.chars().all(|c| !c.is_control()) && !t.is_empty() {
                    if crate::dialogs_smart_art::type_into(app, &t) {
                        continue;
                    }
                    // Typing with a text box selected types into it.
                    crate::objects::enter_text_box(app);
                    type_text(app, &t);
                }
            }
            egui::Event::Ime(egui::ImeEvent::DeleteSurrounding { before_chars, after_chars }) => delete_surrounding(app, before_chars, after_chars),
            egui::Event::Ime(egui::ImeEvent::Preedit { text, active_range_chars }) => {
                app.canvas.ime_preedit = text;
                app.canvas.ime_preedit_range = active_range_chars;
            }
            egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                app.canvas.ime_preedit.clear();
                app.canvas.ime_preedit_range = None;
                if !text.is_empty() && !crate::dialogs_smart_art::type_into(app, &text) {
                    type_text(app, &text);
                }
            }
            egui::Event::Paste(t) => {
                // Mod+Alt+V arrives as a paste too (the shell reads the clipboard): Paste Special.
                let m = ctx.input(|i| i.modifiers);
                let id = match (m.alt, m.shift) {
                    (true, true) => "edit.pasteText",
                    (true, false) => "edit.pasteSpecial",
                    _ => "edit.paste",
                };
                let _ = app.run(id, json!({"text": t}));
            }
            egui::Event::Copy | egui::Event::Cut => {
                let id = if matches!(e, egui::Event::Cut) { "edit.cut" } else { "edit.copy" };
                // `run` puts the copied text on the system clipboard itself.
                let _ = app.run(id, json!({}));
            }
            egui::Event::Key { key, pressed: true, modifiers, .. } => {
                if key == Key::Escape && app.session.painter.is_some() {
                    app.session.painter = None;
                    continue;
                }
                // The lasso selection and Ink Replay take Delete and Escape first.
                if crate::ink::key(app, key) {
                    continue;
                }
                // Escape puts the pen or eraser down (back to selecting).
                if key == Key::Escape && app.session.view.draw.mode != wordcraft_engine::cmd::draw::DrawMode::Select {
                    app.canvas.ink = None;
                    let _ = app.run("draw.select", json!({}));
                    continue;
                }
                if key == Key::Escape && crate::table_pen::stop(app) {
                    continue;
                }
                // File › Options › Advanced: the Insert key switches overtype only when asked to.
                if key == Key::Insert && modifiers.is_none() {
                    if app.session.prefs.insert_key_overtype {
                        let _ = app.run("text.overtype", json!({}));
                    }
                    continue;
                }
                if crate::objects::key(app, key, modifiers) {
                    continue;
                }
                // Shift+F10, Windows' keyboard way to the context menu (#821).
                if is_context_menu_key(key, modifiers) {
                    app.canvas.context_menu_from_keys = true;
                    continue;
                }
                // Escape in a text box (or WordArt) selects the box itself, so Delete then removes
                // it (#1538). The anchor position is just after the box's object character.
                if key == Key::Escape
                    && crate::canvas::in_text_box(app)
                    && let wordcraft_doc::StoryRef::Part(id) = app.session.sel.focus.story
                    && let Some(end) = app.session.doc.text_box_anchor(id)
                    && let Some(off) = end.off.checked_sub(wordcraft_doc::para::OBJ.len_utf8())
                {
                    let start = wordcraft_doc::Pos { off, ..end.clone() };
                    let _ = app.run("select.range", json!({"anchor": start, "focus": end}));
                    continue;
                }
                if key == Key::Escape && matches!(app.session.sel.focus.story, wordcraft_doc::StoryRef::Part(_)) && app.session.sel.is_collapsed() {
                    let _ = app.run("insert.closeHeader", json!({}));
                    continue;
                }
                if is_shift_delete(key, modifiers) {
                    if !app.session.sel.is_collapsed()
                        && let Ok(r) = app.run("edit.cut", json!({}))
                    {
                        crate::system_clipboard::put_result(app, ctx, &r);
                    }
                    continue;
                }
                if leave_full_screen(app, key, modifiers, true) {
                    continue;
                }
                app.canvas.swallow_text = dispatch(app, key, modifiers) && modifiers.alt;
            }
            _ => {}
        }
    }
    app.canvas.super_held = super_key_held;
    app.canvas.modifiers = current_mods;
}

/// Escape leaves Focus or Full Screen View (#1678), then Full Screen (#980), once nothing nearer
/// took it: a pen, a text box, a header, a dialog, a menu or a popup, a drag on the page, and in
/// the document (`canvas`) a selection, which Escape collapses first (`select.collapse`), as in a
/// normal window.
fn leave_full_screen(app: &mut WordApp, key: Key, m: Modifiers, canvas: bool) -> bool {
    let screen = crate::chrome::Screen::of(&app.session.view).exit_command();
    let Some(id) = screen.or(app.session.view.full_screen.then_some("view.fullScreen")) else { return false };
    if key != Key::Escape || !m.is_none() || !app.escape_free {
        return false;
    }
    let c = &app.canvas;
    if c.obj_drag.is_some() || c.table_resize.is_some() || c.table_drag.is_some() || c.link_from.is_some() {
        return false;
    }
    if canvas && !app.session.sel.is_collapsed() {
        return false;
    }
    let _ = app.run(id, json!({"value": false}));
    true
}

/// Alt+numpad character codes are a Windows convention; on macOS Option+digit types its own
/// characters (¡ ™ £…), so they're off there (also in a browser on a Mac).
fn alt_codes_supported(ctx: &egui::Context) -> bool {
    !cfg!(target_os = "macos") && ctx.os() != egui::os::OperatingSystem::Mac
}

/// The digit a key types for an Alt code.
///
/// Word counts only numeric-keypad digits. egui 0.36 reports keypad and top-row digits alike
/// (`Key::Num0`…`Num9`, with the same `physical_key`), and eframe passes no winit key location
/// on, so the top-row digits compose codes too (they type nothing else with Alt alone; Ctrl+Alt+1
/// and keytip digits are untouched).
fn alt_code_digit(key: Key) -> Option<char> {
    let n = match key {
        Key::Num0 => 0,
        Key::Num1 => 1,
        Key::Num2 => 2,
        Key::Num3 => 3,
        Key::Num4 => 4,
        Key::Num5 => 5,
        Key::Num6 => 6,
        Key::Num7 => 7,
        Key::Num8 => 8,
        Key::Num9 => 9,
        _ => return None,
    };
    char::from_digit(n, 10)
}

/// Alt+numpad entry (#961): digits typed with Alt alone held collect; releasing Alt types the
/// character their code names (`text.typeCode`: 0233 → é, 233 → Θ in code page 437). Returns
/// whether the event was part of it (and isn't handled otherwise).
fn alt_code_event(app: &mut WordApp, e: &egui::Event) -> bool {
    let alt_only = |m: &Modifiers| m.alt && !m.ctrl && !m.shift && !m.command && !m.mac_cmd;
    match e {
        egui::Event::Key { key, pressed: true, repeat, modifiers, .. } if alt_only(modifiers) => match alt_code_digit(*key) {
            // A held digit's auto-repeat adds nothing (as on Windows).
            Some(_) if *repeat => {
                app.canvas.swallow_text = true;
                true
            }
            Some(d) => {
                // Codes beyond seven digits name nothing; stop collecting rather than grow.
                if app.canvas.alt_code.len() < 8 {
                    app.canvas.alt_code.push(d);
                }
                // The digit's text (Linux sends it even with Alt held) isn't typed.
                app.canvas.swallow_text = true;
                true
            }
            None => {
                app.canvas.alt_code.clear();
                false
            }
        },
        egui::Event::Key { key: Key::AltLeft | Key::AltRight, pressed: false, .. } => {
            finish_alt_code(app);
            false
        }
        egui::Event::ModifiersChanged(m) if !m.alt => {
            finish_alt_code(app);
            false
        }
        egui::Event::Key { pressed: true, .. } => {
            // Another key with Ctrl/Shift held (or without Alt) ends the entry unfinished.
            app.canvas.alt_code.clear();
            false
        }
        _ => false,
    }
}

/// What an Alt code typed, for the platform's own composed character that may follow.
#[derive(Clone, Debug)]
pub(crate) struct AltCodeEcho {
    /// The character typed.
    typed: String,
    /// The system composes Alt codes itself (Windows): whatever it sends is the code's character.
    system_composes: bool,
    /// The code named a byte of the system's code pages (1–255), so the system's character wins
    /// over ours: it knows the code pages (DBCS ones too) better than our locale guess.
    byte_code: bool,
}

/// Alt released: type the collected code's character, if any.
fn finish_alt_code(app: &mut WordApp) {
    let code = std::mem::take(&mut app.canvas.alt_code);
    if code.is_empty() {
        return;
    }
    crate::objects::enter_text_box(app);
    let system_composes = cfg!(target_os = "windows");
    let mut params = json!({"code": code});
    if system_composes {
        // Windows: the code pages of the system locale (Polish 1250/852, Russian 1251/866…).
        let (ansi, oem) = wordcraft_engine::cmd::text::system_code_pages();
        params = json!({"code": code, "ansi": ansi, "oem": oem});
        // The system's composed character comes as text after the digits' swallowed text.
        app.canvas.swallow_text = false;
    }
    if let Ok(r) = app.run("text.typeCode", params) {
        let byte_code = code.trim_start_matches('0').parse::<u32>().is_ok_and(|v| v <= 255);
        app.canvas.alt_code_echo = r.get("char").and_then(Value::as_str).map(|c| AltCodeEcho { typed: c.to_string(), system_composes, byte_code });
    }
}

/// Typed text (a key's text or an input method's commit). Text replacing what an input method's
/// `DeleteSurrounding` just erased takes the erased text's formatting, never overtypes what
/// follows, and stays in the typing undo group (#1480).
fn type_text(app: &mut WordApp, text: &str) {
    if let Some(props) = app.canvas.ime_join_text.take() {
        app.session.join_next_undo();
        app.session.pending = Some(props);
        let overtype = std::mem::replace(&mut app.session.prefs.overtype, false);
        let typed = app.run("text.insert", json!({"text": text})).is_ok();
        app.session.prefs.overtype = overtype;
        if typed {
            app.session.continue_typing();
        }
    } else {
        let _ = app.run("text.insert", json!({"text": text}));
    }
    note_automatic_change(app);
}

/// AutoCorrect or AutoFormat just changed the text around the caret (its own undo step, which
/// closes the typing group). The web's text agent still holds the text as typed and counts its
/// `DeleteSurrounding` from that, so it is asked to start afresh (`canvas::ime_reset`).
fn note_automatic_change(app: &mut WordApp) {
    if !app.session.typing_open() && app.session.undo_label() == Some("AutoFormat") {
        app.canvas.ime_reset = true;
    }
}

/// An input method's `DeleteSurrounding`: erase `before` characters (Unicode scalar values, as
/// the input method counts them) before the caret and `after` after it, as Backspace does over a
/// selection. The web build's text agent (eframe's hidden `<input>`) sends it, then the new
/// text, whenever its value changes other than by an addition at its end (#1480): Backspace
/// and corrections from phone keyboards, and Chrome's caret jump. The desktop builds never get
/// it (egui-winit 0.36 doesn't send it). The counts come from outside: they stop at the
/// paragraph's ends, so a paragraph mark or list number is never erased. The erasure joins an
/// open typing group and the text that follows joins it too ([`type_text`]), so one Undo takes
/// back the typing with its correction.
/// With a selection (the agent starts afresh on any click or editing key, so it doesn't send
/// one then) nothing is erased: the text that follows replaces the selection as typing does.
/// Nor inside an equation, whose text the agent's characters went to.
fn delete_surrounding(app: &mut WordApp, before: usize, after: usize) {
    let s = &app.session;
    // Inside an equation the document's text isn't what the input method typed into.
    if !s.sel.is_collapsed() || s.math.is_some() {
        return;
    }
    let f = s.sel.focus.clone();
    let Some(text) = s.doc.para_at(&f).map(|p| p.text.as_str()) else { return };
    let (Some(head), Some(tail)) = (text.get(..f.off), text.get(f.off..)) else { return };
    let start = f.off.saturating_sub(head.chars().rev().take(before).map(char::len_utf8).sum::<usize>());
    let end = f.off.saturating_add(tail.chars().take(after).map(char::len_utf8).sum::<usize>());
    if start == end {
        return;
    }
    let join = s.typing_open();
    // The formatting typing at the caret would use: that of the erased text (or set for it).
    let props = s.typing_props();
    app.session.sel =
        wordcraft_engine::Selection { anchor: wordcraft_doc::Pos { off: start, ..f.clone() }, focus: wordcraft_doc::Pos { off: end, ..f.clone() } };
    let caret = f;
    if join {
        app.session.join_next_undo();
    }
    if app.run("text.backspace", json!({})).is_ok() {
        app.canvas.ime_join_text = Some(props);
    } else {
        // Refused (a protected document): leave the caret where it was.
        app.session.sel = wordcraft_engine::Selection::caret(caret);
    }
}

/// Text arriving right after an Alt code typed `echo` (no key pressed between): whether it was
/// the platform's composed character and is handled. The same character isn't typed twice. On
/// Windows the text is always the composed character: for a code up to 255 it replaces ours (the
/// system knows its code pages), above 255 ours stays (Windows reduces it to a byte; Word types
/// the Unicode character).
fn alt_code_echoed(app: &mut WordApp, echo: &AltCodeEcho, text: &str) -> bool {
    if echo.typed == text {
        return true;
    }
    if !echo.system_composes {
        return false;
    }
    if echo.byte_code && text.chars().count() == 1 {
        let s = &app.session;
        let f = s.sel.focus.clone();
        let start = f.off.checked_sub(echo.typed.len());
        let ours =
            start.filter(|_| s.sel.is_collapsed()).filter(|st| s.doc.para_at(&f).and_then(|p| p.text.get(*st..f.off)) == Some(echo.typed.as_str()));
        if let Some(start) = ours {
            app.session.sel = wordcraft_engine::Selection { anchor: wordcraft_doc::Pos { off: start, ..f.clone() }, focus: f };
            let _ = app.run("text.insert", json!({"text": text}));
        }
    }
    true
}

/// Whether a key runs its shortcut with the canvas unfocused (after a ribbon click, say): Mod and
/// Alt shortcuts (Alt+Q reaches Search from anywhere, as in Word, #819) and the function keys.
fn is_global_shortcut(key: Key, m: Modifiers) -> bool {
    m.command || m.alt || is_function_key(key)
}

/// Command shortcuts when the canvas isn't focused but no text field is either.
pub fn global_shortcuts(app: &mut WordApp, ctx: &egui::Context) {
    // Customize Keyboard records the keys pressed; they never run commands meanwhile.
    if app.canvas.focused || ctx.egui_wants_keyboard_input() || matches!(app.dialog, Some(crate::dialogs::Dialog::CustomizeKeyboard { .. })) {
        return;
    }
    let events = ctx.input(|i| i.events.clone());
    for e in events {
        if let egui::Event::Key { key, pressed: true, modifiers, .. } = e {
            if leave_full_screen(app, key, modifiers, false) {
                continue;
            }
            if is_global_shortcut(key, modifiers) {
                dispatch(app, key, modifiers);
            }
        }
    }
}
mod alt_code;
mod canvas;
mod ime;
mod shortcuts;

pub(crate) use alt_code::AltCodeEcho;
pub use canvas::canvas_events;
pub(crate) use ime::observe_ime;
#[cfg(test)]
use ime::{ImeProof, MAX_IME_COMMIT_BYTES, MAX_IME_PREEDIT_BYTES, classify_ime_proof};
pub use shortcuts::{combo, global_shortcuts, key_name};
#[cfg(test)]
use shortcuts::{
    dispatch, is_context_menu_key, is_global_shortcut, is_paste_release, is_paste_special_release, is_shift_delete, paste_release_modifiers,
    shift_variant_params, ui_shortcut,
};

    #[test]
    fn typing_with_smart_art_selected_types_into_an_item() {
        let ctx = egui::Context::default();
        let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), crate::Services::default());
        a.run("insert.smartArt", json!({"layout": "matrix", "items": ["Ada", "Bea"]})).unwrap();
        for t in ["x", "y"] {
            ctx.run_ui(egui::RawInput { events: vec![egui::Event::Text(t.into())], ..Default::default() }, |ui| canvas_events(&mut a, ui.ctx()))
                .drop_without_applying_deltas();
        }
        let g = wordcraft_engine::cmd::smart_art::selected_smart_art(&a.session).expect("the graphic stays");
        assert_eq!(g.items.iter().map(|i| i.text.as_str()).collect::<Vec<_>>(), ["Adaxy", "Bea"]);
        assert!(a.session.view.smart_art_pane, "the Text Pane opens");
        a.run("edit.undo", json!({})).unwrap();
        let g = wordcraft_engine::cmd::smart_art::selected_smart_art(&a.session).expect("still there");
        assert_eq!(g.items[0].text, "Ada", "one undo step for the typing");
    }

    /// A system shortcut with the Super (Win / Logo) key held, such as Win+Space to switch
    /// input methods, must not type a space or any other character into the document.
    #[test]
    fn super_space_does_not_type_space() {
        let doc_text = |a: &WordApp| a.session.doc.plain_text(wordcraft_doc::StoryRef::Body);
        let ctx = egui::Context::default();
        let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), crate::Services::default());
        let none = Modifiers::NONE;
        let key = |key, pressed, modifiers| egui::Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers };
        let frame = |a: &mut WordApp, events: Vec<egui::Event>| {
            ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| {
                crate::keytips::logic(a, ui.ctx());
                canvas_events(a, ui.ctx());
            })
            .drop_without_applying_deltas();
        };

        // Same-frame test: Super-down, Space-down, Text(" "), Space-up, Super-up all in one frame.
        frame(
            &mut a,
            vec![
                key(Key::SuperLeft, true, none),
                key(Key::Space, true, none),
                egui::Event::Text(" ".into()),
                key(Key::Space, false, none),
                key(Key::SuperLeft, false, none),
            ],
        );
        assert_eq!(doc_text(&a), "", "Win+Space within a single frame must not type a space into the document");

        // Cross-frame release: Super down in frame 1; Space-down, Text(" "), Space-up, and Super-up in frame 2.
        // In egui, `ctx.input(|i| i.key_down(Key::SuperLeft))` is false in frame 2 because it is released
        // within frame 2; the prior-frame state in app.canvas.super_held must preserve it until the release event.
        frame(&mut a, vec![key(Key::SuperLeft, true, none)]);
        frame(
            &mut a,
            vec![key(Key::Space, true, none), egui::Event::Text(" ".into()), key(Key::Space, false, none), key(Key::SuperLeft, false, none)],
        );
        assert_eq!(doc_text(&a), "", "Space before Super-up in the next frame must not type a space into the document");

        // Across multiple frames: Super down in frame 1, Space+Text in frame 2, Super up in frame 3.
        frame(&mut a, vec![key(Key::SuperLeft, true, none)]);
        frame(&mut a, vec![key(Key::Space, true, none), egui::Event::Text(" ".into()), key(Key::Space, false, none)]);
        frame(&mut a, vec![key(Key::SuperLeft, false, none)]);
        assert_eq!(doc_text(&a), "", "Win+Space across frames must not type a space into the document");

        // Normal typing after releasing Super still works.
        frame(&mut a, vec![key(Key::A, true, none), egui::Event::Text("a".into()), key(Key::A, false, none)]);
        assert_eq!(doc_text(&a).trim_end(), "a", "regular typing after Super release works");

        // Compositor swallows Super-up: ModifiersChanged(none) clears super_held.
        frame(&mut a, vec![key(Key::SuperLeft, true, none)]);
        // Compositor consumes Super release, sends ModifiersChanged with no modifiers.
        frame(&mut a, vec![egui::Event::ModifiersChanged(none)]);
        frame(&mut a, vec![key(Key::B, true, none), egui::Event::Text("b".into()), key(Key::B, false, none)]);
        assert_eq!(doc_text(&a).trim_end(), "ab", "typing works after swallowed Super release cleared by ModifiersChanged");

        // Super held while Shift is pressed and released: ModifiersChanged(none) from Shift release
        // must not clear super_key_held; subsequent Super+Space must not type a space.
        let shift = egui::Modifiers { shift: true, ..Default::default() };
        frame(&mut a, vec![key(Key::SuperLeft, true, none)]);
        frame(&mut a, vec![egui::Event::ModifiersChanged(shift)]);
        frame(&mut a, vec![egui::Event::ModifiersChanged(none)]);
        frame(&mut a, vec![key(Key::Space, true, none), egui::Event::Text(" ".into()), key(Key::Space, false, none)]);
        assert_eq!(doc_text(&a).trim_end(), "ab", "releasing Shift while Super is held must not allow Super+Space to type space");
        // Same-frame Shift press and release while Super is held.
        frame(
            &mut a,
            vec![
                egui::Event::ModifiersChanged(shift),
                egui::Event::ModifiersChanged(none),
                key(Key::Space, true, none),
                egui::Event::Text(" ".into()),
                key(Key::Space, false, none),
            ],
        );
        assert_eq!(doc_text(&a).trim_end(), "ab", "same-frame Shift release while Super is held must not type space");
        // Releasing Super allows typing again.
        frame(&mut a, vec![key(Key::SuperLeft, false, none)]);
        frame(&mut a, vec![key(Key::C, true, none), egui::Event::Text("c".into()), key(Key::C, false, none)]);
        assert_eq!(doc_text(&a).trim_end(), "abc", "typing resumes after Super is released");
    }
}
#[cfg(test)]
mod tests;
