use kesa::theme::Theme;

fn srgb_to_linear(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn relative_luminance(hex: &str) -> f64 {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).expect("valid hex color");
    let g = u8::from_str_radix(&hex[2..4], 16).expect("valid hex color");
    let b = u8::from_str_radix(&hex[4..6], 16).expect("valid hex color");
    0.2126_f64.mul_add(
        srgb_to_linear(r),
        0.7152_f64.mul_add(srgb_to_linear(g), 0.0722 * srgb_to_linear(b)),
    )
}

fn contrast_ratio(a: &str, b: &str) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (l1, l2) = if la > lb { (la, lb) } else { (lb, la) };
    (l1 + 0.05) / (l2 + 0.05)
}

fn assert_floor(theme_name: &str, token: &str, fg: &str, bg: &str, floor: f64) {
    let ratio = contrast_ratio(fg, bg);
    assert!(
        ratio >= floor,
        "{theme_name} {token} {fg} on {bg} is {ratio:.2}:1, floor {floor:.1}:1"
    );
}

fn check_theme(theme: &Theme) {
    let bg = &theme.colors.background;
    let name = &theme.name;

    assert_floor(name, "ui.border", &theme.ui.border, bg, 3.0);

    assert_floor(name, "foreground", &theme.colors.foreground, bg, 4.5);
    assert_floor(name, "muted", &theme.colors.muted, bg, 4.5);
    assert_floor(name, "accent", &theme.colors.accent, bg, 4.5);
    assert_floor(name, "success", &theme.colors.success, bg, 4.5);
    assert_floor(name, "warning", &theme.colors.warning, bg, 4.5);
    assert_floor(name, "error", &theme.colors.error, bg, 4.5);

    assert_floor(name, "syntax.keyword", &theme.syntax.keyword, bg, 4.5);
    assert_floor(name, "syntax.string", &theme.syntax.string, bg, 4.5);
    assert_floor(name, "syntax.number", &theme.syntax.number, bg, 4.5);
    assert_floor(name, "syntax.comment", &theme.syntax.comment, bg, 4.5);
    assert_floor(name, "syntax.function", &theme.syntax.function, bg, 4.5);
}

#[test]
fn dark_theme_clears_contrast_floors() {
    check_theme(&Theme::dark());
}

#[test]
fn light_theme_clears_contrast_floors() {
    check_theme(&Theme::light());
}

#[test]
fn solarized_theme_clears_contrast_floors() {
    check_theme(&Theme::solarized());
}

#[test]
fn old_dark_border_would_have_failed_the_gate() {
    let ratio = contrast_ratio("#3c3c3c", "#1e1e1e");
    assert!(
        ratio < 3.0,
        "expected the pre-redesign dark border to fail the 3.0 floor, measured {ratio:.2}:1"
    );
}

fn tokens(theme: &Theme) -> Vec<(&'static str, String)> {
    vec![
        ("ui.border", theme.ui.border.clone()),
        ("foreground", theme.colors.foreground.clone()),
        ("muted", theme.colors.muted.clone()),
        ("accent", theme.colors.accent.clone()),
        ("success", theme.colors.success.clone()),
        ("warning", theme.colors.warning.clone()),
        ("error", theme.colors.error.clone()),
        ("syntax.keyword", theme.syntax.keyword.clone()),
        ("syntax.string", theme.syntax.string.clone()),
        ("syntax.number", theme.syntax.number.clone()),
        ("syntax.comment", theme.syntax.comment.clone()),
        ("syntax.function", theme.syntax.function.clone()),
    ]
}

// Run with: cargo test --test theme_contrast -- --ignored --nocapture contrast_table
#[test]
#[ignore = "prints a table for the report rather than asserting anything"]
fn contrast_table() {
    let mut rows = Vec::new();
    for theme in [Theme::dark(), Theme::light(), Theme::solarized()] {
        let bg = theme.colors.background.clone();
        for (token, hex) in tokens(&theme) {
            let floor = if token == "ui.border" { 3.0 } else { 4.5 };
            rows.push((
                contrast_ratio(&hex, &bg),
                theme.name.clone(),
                token,
                hex,
                bg.clone(),
                floor,
            ));
        }
    }
    rows.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("ratios are finite"));

    println!(
        "{:<10} {:<15} {:<8} {:<8} {:<8} {}",
        "theme", "token", "fg", "bg", "ratio", "floor"
    );
    for (ratio, theme_name, token, hex, bg, floor) in rows {
        println!("{theme_name:<10} {token:<15} {hex:<8} {bg:<8} {ratio:<8.2} {floor:.1}");
    }
}

// Run with: cargo test --test theme_contrast --features tui -- --ignored --nocapture d40
#[cfg(feature = "tui")]
#[test]
#[ignore = "prints a before/after dump for the report rather than asserting anything"]
fn d40_fenced_block_gets_margin_and_its_own_style() {
    let markdown = "An inline `code span` next to a fenced block:\n\n```\nlet x = 1;\n```\n";

    let mut before = kesa::theme::Theme::dark().glamour_style_config();
    before.code_block.block.style.color = before.code.style.color.clone();
    before.code_block.block.style.background_color = None;
    before.code_block.block.margin = None;

    let after = kesa::theme::Theme::dark().glamour_style_config();

    let rendered_before = glamour::Renderer::new()
        .with_style_config(before)
        .render(markdown);
    let rendered_after = glamour::Renderer::new()
        .with_style_config(after)
        .render(markdown);

    println!("--- before (pre-D40: block shares inline's color, no margin) ---");
    println!("{rendered_before:?}");
    println!("--- after (D40: block has its own color/background/margin) ---");
    println!("{rendered_after:?}");
}
