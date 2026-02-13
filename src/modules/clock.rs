use chrono::{Datelike, Local};
use gtk4::prelude::*;
use gtk4::{Button, EventControllerMotion, Label, Popover, PositionType};
use std::time::Duration;

pub fn init(container: &gtk4::Box) {
    // 1. Create a Button instead of a Label
    let button = Button::builder()
        .label(format!("  {}", Local::now().format("%a %d %b %H:%M")))
        .has_frame(false) // Makes it look flat, common for status bars
        .build();

    button.add_css_class("btn");

    container.append(&button);

    // 2. Setup the Popover
    let popover = Popover::builder()
        .position(PositionType::Top)
        .autohide(false)
        .has_arrow(true)
        .build();

    // Set the button as the parent of the popover
    popover.set_parent(&button);

    let popover_label = Label::builder()
        .use_markup(true)
        .label(get_calendar_markup())
        .build();
    popover_label.set_widget_name("popover-label");
    popover.set_child(Some(&popover_label));

    // 3. Hover logic using EventControllerMotion on the Button
    let motion_controller = EventControllerMotion::new();

    let p_enter = popover.clone();
    motion_controller.connect_enter(move |_, _, _| {
        p_enter.popup();
    });

    let p_leave = popover.clone();
    motion_controller.connect_leave(move |_| {
        p_leave.popdown();
    });

    button.add_controller(motion_controller);

    // 4. Update loop
    let p_label = popover_label.clone();
    let b_clone = button.clone();
    glib::timeout_add_local(Duration::from_secs(1), move || {
        // Update the button's text
        b_clone.set_label(&format!("  {}", Local::now().format("%a %d %b %H:%M")));

        // Update the calendar inside the popover
        p_label.set_markup(&get_calendar_markup());

        glib::ControlFlow::Continue
    });
}

fn get_calendar_markup() -> String {
    let now = Local::now();
    let year = now.year();

    let mut full_markup = String::from("<tt><small>");

    for row in 0..4 {
        let mut lines = vec![String::new(); 9];

        for col in 0..3 {
            let month = (row * 3 + col + 1) as u32;
            let month_date = now.with_month(month).unwrap().with_day(1).unwrap();
            let month_name = month_date.format("%B").to_string();

            let padding = (20 - month_name.len()) / 2;
            lines[0].push_str(&format!(
                "{:>width$}{:<width$}",
                "",
                month_name,
                width = padding
            ));
            if !month_name.len().wrapping_rem(2) == 0 && !lines[0].len().wrapping_rem(20) == 0 {
                lines[0].push(' ');
            }
            while lines[0].len() % 22 != 0 {
                lines[0].push(' ');
            }

            lines[1].push_str("<span color='#ffcc66'>Mo Tu We Th Fr Sa Su</span>  ");

            let weekday = month_date.weekday().num_days_from_monday();
            let days_in_month = get_days_in_month(year, month);

            let mut current_day = 1;
            for week in 0..6 {
                let line_idx = week + 2;
                for d in 0..7 {
                    if (week == 0 && d < weekday) || current_day > days_in_month {
                        lines[line_idx].push_str("   ");
                    } else {
                        let day_str = if month == now.month() && current_day == now.day() {
                            format!(
                                "<span color='#ff6699'><b><u>{:2}</u></b></span> ",
                                current_day
                            )
                        } else {
                            format!("{:2} ", current_day)
                        };
                        lines[line_idx].push_str(&day_str);
                        current_day += 1;
                    }
                }
                lines[line_idx].push(' ');
            }
        }

        for line in lines {
            if !line.trim().is_empty() {
                full_markup.push_str(&line);
                full_markup.push('\n');
            }
        }
        full_markup.push('\n');
    }

    full_markup.push_str("</small></tt>");
    full_markup
}

fn get_days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}
