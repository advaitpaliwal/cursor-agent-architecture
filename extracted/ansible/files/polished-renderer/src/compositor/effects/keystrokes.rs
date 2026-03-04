use std::collections::HashMap;

use font_kit::family_name::FamilyName;
use font_kit::properties::{Properties, Weight};
use font_kit::source::SystemSource;
use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use fontdue::{Font, FontSettings};
use tracing::warn;

use crate::compositor::frame::RgbaFrame;
use crate::easing::bezier::cubic_bezier;
use crate::error::{RendererError, Result};
use crate::plan::types::{KeystrokeEvent, KeystrokeEventType};

const FADE_IN_MS: f64 = 150.0;
const HOLD_DURATION_MS: f64 = 1200.0;
const FADE_OUT_MS: f64 = 400.0;

#[derive(Debug)]
pub struct KeystrokeTimeline {
    events: Vec<KeystrokeDisplayEvent>,
}

#[derive(Debug)]
struct KeystrokeDisplayEvent {
    show_start_ms: f64,
    video_timestamp_ms: f64,
    display_text: String,
}

#[derive(Debug, Clone, Copy)]
pub struct KeystrokeState<'a> {
    pub display_text: &'a str,
    pub opacity: f64,
}

impl KeystrokeTimeline {
    pub fn new(events: &[KeystrokeEvent]) -> Self {
        let mut displayable = Vec::new();
        for event in events {
            if event.event_type == KeystrokeEventType::TextTyped {
                continue;
            }
            let Some(display_text) = format_key_display(&event.display_text) else {
                continue;
            };
            displayable.push(KeystrokeDisplayEvent {
                show_start_ms: event.video_timestamp_ms - FADE_IN_MS,
                video_timestamp_ms: event.video_timestamp_ms,
                display_text,
            });
        }

        displayable.sort_by(|a, b| a.video_timestamp_ms.total_cmp(&b.video_timestamp_ms));
        Self {
            events: displayable,
        }
    }

    pub fn state_at(&self, time_ms: f64) -> Option<KeystrokeState<'_>> {
        if self.events.is_empty() {
            return None;
        }

        let mut active_idx: Option<usize> = None;
        for (i, event) in self.events.iter().enumerate() {
            if time_ms >= event.show_start_ms {
                active_idx = Some(i);
            } else {
                break;
            }
        }

        let idx = active_idx?;
        let event = &self.events[idx];
        let next_event_time = self.events.get(idx + 1).map(|e| e.show_start_ms);

        if let Some(next_time) = next_event_time {
            if time_ms >= next_time {
                return None;
            }
        }

        let elapsed = time_ms - event.show_start_ms;
        let total_duration = FADE_IN_MS + HOLD_DURATION_MS + FADE_OUT_MS;
        if elapsed > total_duration {
            return None;
        }

        let fade_out_start = FADE_IN_MS + HOLD_DURATION_MS;
        let opacity = if elapsed <= FADE_IN_MS {
            remotion_ease(elapsed / FADE_IN_MS)
        } else if elapsed > fade_out_start {
            let t = ((elapsed - fade_out_start) / FADE_OUT_MS).clamp(0.0, 1.0);
            remotion_ease(1.0 - t)
        } else {
            1.0
        };

        Some(KeystrokeState {
            display_text: &event.display_text,
            opacity,
        })
    }
}

pub struct KeystrokeRenderer {
    output_width: u32,
    output_height: u32,
    scale: f64,
    font: Font,
    cache: HashMap<String, KeystrokeOverlayImage>,
}

pub struct KeystrokeOverlayImage {
    pub frame: RgbaFrame,
    pub content_offset_x: i64,
    pub content_offset_y: i64,
    pub pill_width: u32,
    pub pill_height: u32,
}

impl KeystrokeRenderer {
    pub fn new(output_width: u32, output_height: u32) -> Result<Self> {
        let scale = (output_width as f64 / 1920.0) * 2.0;
        let font = load_system_font()?;
        Ok(Self {
            output_width,
            output_height,
            scale,
            font,
            cache: HashMap::new(),
        })
    }

    pub fn overlay_y_plane(&mut self, y_plane: &mut [u8], state: KeystrokeState<'_>) -> Result<()> {
        if state.opacity <= 0.0 {
            return Ok(());
        }

        let output_width_u32 = self.output_width;
        let output_height_u32 = self.output_height;
        let output_width = output_width_u32 as i64;
        let output_height = output_height_u32 as i64;
        let scale = self.scale;

        let expected = (output_width_u32 as usize).saturating_mul(output_height_u32 as usize);
        if y_plane.len() != expected {
            return Err(RendererError::InvalidArgument(format!(
                "KeystrokeRenderer y_plane has wrong length (expected {expected}, got {})",
                y_plane.len()
            )));
        }

        let display_text = sanitize_display_text_for_font(state.display_text, &self.font);
        let overlay = self.get_or_render(&display_text)?;

        let bottom = (60.0 * scale).round() as i64;

        let pill_x = (output_width - overlay.pill_width as i64) / 2;
        let pill_y = output_height - bottom - overlay.pill_height as i64;

        let x = pill_x - overlay.content_offset_x;
        let y = pill_y - overlay.content_offset_y;
        overlay_rgba_into_y_plane(
            y_plane,
            output_width_u32,
            output_height_u32,
            &overlay.frame,
            x,
            y,
            state.opacity,
        );
        Ok(())
    }

    fn get_or_render(&mut self, display_text: &str) -> Result<&KeystrokeOverlayImage> {
        if !self.cache.contains_key(display_text) {
            let rendered = render_keystroke_overlay(display_text, self.scale, &self.font)?;
            self.cache.insert(display_text.to_owned(), rendered);
        }
        self.cache
            .get(display_text)
            .ok_or_else(|| RendererError::Other("Keystroke cache insert failed".into()))
    }
}

fn overlay_rgba_into_y_plane(
    y_plane: &mut [u8],
    dst_width: u32,
    dst_height: u32,
    src: &RgbaFrame,
    x: i64,
    y: i64,
    opacity: f64,
) {
    if dst_width == 0 || dst_height == 0 {
        return;
    }
    let expected = (dst_width as usize).saturating_mul(dst_height as usize);
    if y_plane.len() != expected {
        return;
    }

    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }

    let dst_row_len = dst_width as usize;
    for sy in 0..src.height as i64 {
        let dy = y + sy;
        if dy < 0 || dy >= dst_height as i64 {
            continue;
        }
        let dy_u = dy as usize;
        let dst_row = &mut y_plane[dy_u * dst_row_len..(dy_u + 1) * dst_row_len];

        for sx in 0..src.width as i64 {
            let dx = x + sx;
            if dx < 0 || dx >= dst_width as i64 {
                continue;
            }
            let i = ((sy as u32) * src.width + (sx as u32)) as usize * 4;
            if i + 3 >= src.data.len() {
                continue;
            }
            let a = (src.data[i + 3] as f64 / 255.0) * opacity;
            if a <= 0.0 {
                continue;
            }

            let r = src.data[i] as f64;
            let g = src.data[i + 1] as f64;
            let b = src.data[i + 2] as f64;
            let luma = (0.2126 * r + 0.7152 * g + 0.0722 * b)
                .round()
                .clamp(0.0, 255.0);
            let src_y = 16.0 + luma * (219.0 / 255.0);

            let dx_u = dx as usize;
            let dst_y = dst_row[dx_u] as f64;
            dst_row[dx_u] = (src_y * a + dst_y * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
        }
    }
}

fn load_system_font() -> Result<Font> {
    let source = SystemSource::new();
    let properties = Properties {
        weight: Weight::MEDIUM,
        ..Properties::new()
    };

    let families = [
        "SF Pro Text",
        "SF Pro Display",
        "San Francisco",
        "Helvetica Neue",
        "Helvetica",
        "Arial",
        "Liberation Sans",
        "DejaVu Sans",
        "Noto Sans",
        "Noto Sans Symbols2",
        "Noto Sans Symbols",
        "Segoe UI Symbol",
    ];

    // Symbols we use in keystroke rendering/formatting. If a font can't render these, we'd
    // rather keep searching for a better one (or fall back to ASCII later).
    const REQUIRED_GLYPHS: &[char] = &[
        '⌘', '⌥', '⇧', '⏎', '↵', '⇥', '⌫', '⌦', '␣', '⎋', '↑', '↓', '←', '→', '⇱', '⇲', '⇞', '⇟',
    ];

    let mut best: Option<(Font, usize, &'static str)> = None;

    for family in families {
        let handle =
            source.select_best_match(&[FamilyName::Title(family.to_string())], &properties);
        let Ok(handle) = handle else {
            continue;
        };
        let Ok(font) = handle.load() else {
            continue;
        };
        let Some(data) = font.copy_font_data() else {
            continue;
        };
        let Ok(parsed) = Font::from_bytes(data.as_slice(), FontSettings::default()) else {
            continue;
        };

        // Avoid selecting symbol-only fonts as the primary keystroke font.
        // We render alphanumerics too (e.g., "⌘+S"), so the chosen font must cover basic ASCII.
        if !font_has_glyph(&parsed, 'A') || !font_has_glyph(&parsed, '0') {
            continue;
        }

        let missing = REQUIRED_GLYPHS
            .iter()
            .filter(|&&ch| !font_has_glyph(&parsed, ch))
            .count();
        if missing == 0 {
            return Ok(parsed);
        }

        match best {
            None => best = Some((parsed, missing, family)),
            Some((_, best_missing, _)) if missing < best_missing => {
                best = Some((parsed, missing, family))
            }
            Some(_) => {}
        }
    }

    if let Some((font, missing, family)) = best {
        warn!(
            "Keystroke overlay font '{}' is missing {missing} symbol glyphs; will degrade to ASCII for those. To fix, install a font with these glyphs (e.g. SF Pro Text, Noto Sans Symbols2) and rebuild font cache (fc-cache -f).",
            family
        );
        return Ok(font);
    }

    Err(RendererError::Validation(
        "No usable system font found for keystroke overlay; install fontconfig + a sans-serif font (e.g., DejaVu Sans or Noto Sans) and run `fc-cache -f`".into(),
    ))
}

fn font_has_glyph(font: &Font, ch: char) -> bool {
    // `0` is typically `.notdef` for missing glyphs.
    font.lookup_glyph_index(ch) != 0
}

fn sanitize_display_text_for_font(text: &str, font: &Font) -> String {
    let mut out = text.to_string();

    // Modifiers: keep semantic meaning even when the symbol isn't supported.
    // Use ASCII tokens; include '+' to keep combo parsing/styling consistent.
    if !font_has_glyph(font, '⌘') && out.contains('⌘') {
        out = out.replace('⌘', "Cmd+");
    }
    if !font_has_glyph(font, '⌥') && out.contains('⌥') {
        out = out.replace('⌥', "Alt+");
    }
    if !font_has_glyph(font, '⇧') && out.contains('⇧') {
        out = out.replace('⇧', "Shift+");
    }

    // Non-modifier symbols: prefer dropping the glyph and keeping the ASCII label (if present),
    // otherwise fall back to a readable ASCII word.
    for (sym, fallback) in [
        ('⏎', "Enter"),
        ('↵', "Enter"),
        ('⇥', "Tab"),
        ('⎋', "Esc"),
        ('⌫', "Backspace"),
        ('⌦', "Delete"),
        ('␣', "Space"),
        ('↑', "Up"),
        ('↓', "Down"),
        ('←', "Left"),
        ('→', "Right"),
        ('⇱', "Home"),
        ('⇲', "End"),
        ('⇞', "PgUp"),
        ('⇟', "PgDn"),
    ] {
        if !out.contains(sym) || font_has_glyph(font, sym) {
            continue;
        }

        let removed = out.replace(sym, "");
        let cleaned = removed.split_whitespace().collect::<Vec<_>>().join(" ");
        let trimmed = cleaned.trim();
        out = if trimmed.is_empty() {
            fallback.to_string()
        } else if trimmed.ends_with('+') {
            format!("{trimmed}{fallback}")
        } else {
            cleaned
        };
    }

    // Collapse accidental "++" that can occur when we add modifier '+' and the input already had '+'.
    while out.contains("++") {
        out = out.replace("++", "+");
    }
    out = out.trim_matches('+').trim().to_string();
    out
}

pub fn format_key_display(text: &str) -> Option<String> {
    if text == "\n" || text == "\r" || text == "\r\n" {
        return None;
    }

    let mut result = text.trim().to_string();
    if let Some(stripped) = result
        .strip_prefix('"')
        .or_else(|| result.strip_prefix('\''))
    {
        result = stripped.to_string();
    }
    if let Some(stripped) = result
        .strip_suffix('"')
        .or_else(|| result.strip_suffix('\''))
    {
        result = stripped.to_string();
    }
    result = result.trim().to_string();
    if result.is_empty() {
        return None;
    }

    // If text already contains symbols, we may need to fix legacy ⌃ -> ^
    let existing_symbols = [
        "↵", "⏎", "⇥", "⌫", "⌦", "␣", "↑", "↓", "←", "→", "⌘", "⌃", "⌥", "⇧",
    ];
    if existing_symbols.iter().any(|sym| result.contains(sym)) {
        // Fix legacy ⌃ to ^ (for old recording-data.json files)
        result = result.replace("⌃", "^");

        let removable = [
            "Enter",
            "Return",
            "Tab",
            "Backspace",
            "Delete",
            "Space",
            "Up",
            "Down",
            "Left",
            "Right",
            "Cmd",
            "Command",
            "Ctrl",
            "Control",
            "Alt",
            "Option",
            "Shift",
        ];
        let filtered: Vec<&str> = result
            .split_whitespace()
            .filter(|token| !removable.iter().any(|w| token.eq_ignore_ascii_case(w)))
            .collect();
        let cleaned = filtered.join(" ").trim().to_string();
        if !cleaned.is_empty() {
            return Some(cleaned);
        }
        return None;
    }

    let mut replaced = result;
    for (key, symbol) in [
        ("Return", "⏎"),
        ("Enter", "⏎"),
        ("Tab", "⇥"),
        ("Escape", "Esc"),
        ("Backspace", "⌫"),
        ("Delete", "⌦"),
        ("space", "␣"),
        ("Up", "↑"),
        ("Down", "↓"),
        ("Left", "←"),
        ("Right", "→"),
        ("cmd", "⌘"),
        ("command", "⌘"),
        ("ctrl", "^"),
        ("control", "^"),
        ("alt", "⌥"),
        ("option", "⌥"),
        ("shift", "⇧"),
    ] {
        replaced = replace_word_boundary_ascii_case_insensitive(&replaced, key, symbol);
    }

    if replaced.trim().is_empty() {
        return None;
    }

    Some(replaced.trim().to_string())
}

fn replace_word_boundary_ascii_case_insensitive(
    input: &str,
    word: &str,
    replacement: &str,
) -> String {
    if word.is_empty() {
        return input.to_string();
    }

    let word_bytes = word.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        if i + word_bytes.len() <= bytes.len()
            && bytes[i..i + word_bytes.len()].eq_ignore_ascii_case(word_bytes)
        {
            let before = i.checked_sub(1).map(|idx| bytes[idx]);
            let after = (i + word_bytes.len() < bytes.len()).then(|| bytes[i + word_bytes.len()]);

            if is_word_boundary(before) && is_word_boundary(after) {
                out.extend_from_slice(replacement.as_bytes());
                i += word_bytes.len();
                continue;
            }
        }

        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

fn is_word_boundary(byte: Option<u8>) -> bool {
    byte.map_or(true, |b| !is_word_byte(b))
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn remotion_ease(t: f64) -> f64 {
    // Remotion's `Easing.ease` matches CSS `ease`: cubic-bezier(0.25, 0.1, 0.25, 1.0)
    cubic_bezier(0.25, 0.1, 0.25, 1.0, t.clamp(0.0, 1.0))
}

fn render_keystroke_overlay(text: &str, scale: f64, font: &Font) -> Result<KeystrokeOverlayImage> {
    let is_combo = text.contains('+') || ["⌘", "⌃", "⌥", "⇧"].iter().any(|m| text.contains(m));
    let typography_scale = 0.85;

    let padding_y = if is_combo { 18.0 } else { 16.0 } * scale;
    let padding_x = if is_combo { 28.0 } else { 24.0 } * scale;
    let gap = 10.0 * scale;
    let font_size_main = (if is_combo { 28.0 } else { 26.0 } * scale * typography_scale).max(1.0);
    let font_size_plus = (22.0 * scale * typography_scale).max(1.0);

    let parts: Vec<&str> = text
        .split('+')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return Err(RendererError::Validation(
            "Keystroke overlay has empty text".into(),
        ));
    }

    #[derive(Clone)]
    struct Segment {
        image: RgbaFrame,
    }

    let mut segments = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        let part_img = render_text_segment(
            font,
            part,
            font_size_main as f32,
            [255, 255, 255, (0.95_f64 * 255.0).round() as u8],
            Some(TextShadow {
                offset_y: (1.0 * scale).round() as i32,
                blur: 0,
                color: [0, 0, 0, (0.30_f64 * 255.0).round() as u8],
            }),
        )?;
        segments.push(Segment { image: part_img });
        if i < parts.len() - 1 {
            let plus_img = render_text_segment(
                font,
                "+",
                font_size_plus as f32,
                [255, 255, 255, (0.40_f64 * 255.0).round() as u8],
                None,
            )?;
            segments.push(Segment { image: plus_img });
        }
    }

    let content_height = segments
        .iter()
        .map(|s| s.image.height)
        .max()
        .unwrap_or(0)
        .max(1);

    let content_width_f: f64 = segments.iter().map(|s| s.image.width as f64).sum::<f64>()
        + gap * (segments.len().saturating_sub(1) as f64);

    let pill_width = (padding_x * 2.0 + content_width_f).ceil().max(2.0) as u32;
    let pill_height = (padding_y * 2.0 + content_height as f64).ceil().max(2.0) as u32;

    let corner_radius = (16.0 * scale).round().max(1.0);
    let shadow_offset_y = (3.0 * scale).round().max(0.0) as u32;
    let shadow_blur = (12.0 * scale).round().max(1.0) as u32;
    let shadow_passes = 3usize;
    let shadow_supersample = 2u32;
    // Box blur spreads by roughly `radius` per pass, so reserve full falloff area to avoid clipping.
    let shadow_margin = shadow_blur
        .saturating_mul(shadow_passes as u32)
        .saturating_add(2);
    let margin_left = shadow_margin;
    let margin_right = shadow_margin;
    let margin_top = shadow_margin;
    let margin_bottom = shadow_margin + shadow_offset_y;

    let out_width = pill_width + margin_left + margin_right;
    let out_height = pill_height + margin_top + margin_bottom;

    let mut out = RgbaFrame::new(out_width, out_height);

    let content_offset_x = margin_left as i64;
    let content_offset_y = margin_top as i64;

    if shadow_blur > 0 || shadow_offset_y > 0 {
        let mask_width = out_width.saturating_mul(shadow_supersample);
        let mask_height = out_height.saturating_mul(shadow_supersample);
        let mut mask = vec![0u8; (mask_width as usize).saturating_mul(mask_height as usize)];
        draw_rounded_rect_mask(
            &mut mask,
            mask_width,
            mask_height,
            content_offset_x.saturating_mul(shadow_supersample as i64),
            (content_offset_y + shadow_offset_y as i64).saturating_mul(shadow_supersample as i64),
            pill_width.saturating_mul(shadow_supersample),
            pill_height.saturating_mul(shadow_supersample),
            corner_radius * shadow_supersample as f64,
        );
        blur_mask_box(
            &mut mask,
            mask_width as usize,
            mask_height as usize,
            (shadow_blur.saturating_mul(shadow_supersample)) as usize,
            shadow_passes,
        );
        apply_shadow_supersampled(
            &mut out,
            &mask,
            mask_width as usize,
            mask_height as usize,
            shadow_supersample as usize,
            [0, 0, 0],
            (0.70_f64 * 255.0).round() as u8,
        );
    }

    let pill_x = content_offset_x as i64;
    let pill_y = content_offset_y as i64;

    draw_rounded_rect(
        &mut out,
        pill_x,
        pill_y,
        pill_width,
        pill_height,
        corner_radius,
        [20, 20, 20, (0.9_f64 * 255.0).round() as u8],
    );

    draw_rounded_rect_border(
        &mut out,
        pill_x,
        pill_y,
        pill_width,
        pill_height,
        corner_radius,
        2,
        [77, 77, 77, (0.9_f64 * 255.0).round() as u8],
    );

    let mut cursor_x = pill_x as f64 + padding_x;
    let center_y = pill_y as f64 + padding_y + (content_height as f64) / 2.0;
    for (idx, seg) in segments.iter().enumerate() {
        let seg_y = center_y - (seg.image.height as f64) / 2.0;
        overlay_at(
            &mut out,
            &seg.image,
            cursor_x.round() as i64,
            seg_y.round() as i64,
        );
        cursor_x += seg.image.width as f64;
        if idx + 1 < segments.len() {
            cursor_x += gap;
        }
    }

    Ok(KeystrokeOverlayImage {
        frame: out,
        content_offset_x,
        content_offset_y,
        pill_width,
        pill_height,
    })
}

#[derive(Debug, Clone, Copy)]
struct TextShadow {
    offset_y: i32,
    blur: u32,
    color: [u8; 4],
}

fn render_text_segment(
    font: &Font,
    text: &str,
    font_size_px: f32,
    color: [u8; 4],
    shadow: Option<TextShadow>,
) -> Result<RgbaFrame> {
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings {
        x: 0.0,
        y: 0.0,
        ..LayoutSettings::default()
    });
    layout.append(&[font], &TextStyle::new(text, font_size_px, 0));
    let glyphs = layout.glyphs();
    if glyphs.is_empty() {
        return Ok(RgbaFrame::new(1, 1));
    }

    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    let mut rasters = Vec::with_capacity(glyphs.len());
    for glyph in glyphs {
        let (metrics, bitmap) = font.rasterize_config(glyph.key);
        let gx0 = glyph.x;
        let gy0 = glyph.y;
        let gx1 = gx0 + metrics.width as f32;
        let gy1 = gy0 + metrics.height as f32;
        min_x = min_x.min(gx0);
        min_y = min_y.min(gy0);
        max_x = max_x.max(gx1);
        max_y = max_y.max(gy1);
        rasters.push((glyph.x, glyph.y, metrics.width, metrics.height, bitmap));
    }

    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        return Ok(RgbaFrame::new(1, 1));
    }

    let width = (max_x - min_x).ceil().max(1.0) as u32;
    let height = (max_y - min_y).ceil().max(1.0) as u32;
    let mut out = RgbaFrame::new(width, height);

    if let Some(shadow) = shadow {
        draw_glyphs(
            &mut out,
            &rasters,
            -min_x,
            -min_y + shadow.offset_y as f32,
            shadow.color,
        );
        if shadow.blur > 0 {
            // Reserved for future blur; for now shadow.blur is always 0.
        }
    }

    draw_glyphs(&mut out, &rasters, -min_x, -min_y, color);
    Ok(out)
}

fn draw_glyphs(
    out: &mut RgbaFrame,
    glyphs: &[(f32, f32, usize, usize, Vec<u8>)],
    offset_x: f32,
    offset_y: f32,
    color: [u8; 4],
) {
    for (gx, gy, w, h, bitmap) in glyphs {
        for row in 0..*h {
            for col in 0..*w {
                let cov = bitmap[row * *w + col] as f64 / 255.0;
                if cov <= 0.0 {
                    continue;
                }
                let x = (*gx + offset_x + col as f32).round() as i64;
                let y = (*gy + offset_y + row as f32).round() as i64;
                if x < 0 || y < 0 {
                    continue;
                }
                let x = x as u32;
                let y = y as u32;
                if x >= out.width || y >= out.height {
                    continue;
                }
                let a = (color[3] as f64 * cov).round().clamp(0.0, 255.0) as u8;
                if a == 0 {
                    continue;
                }
                let src = [color[0], color[1], color[2], a];
                let dst = out.get_pixel(x, y);
                out.set_pixel(x, y, alpha_blend(dst, src));
            }
        }
    }
}

fn overlay_at(dst: &mut RgbaFrame, src: &RgbaFrame, x: i64, y: i64) {
    overlay_with_opacity(dst, src, x, y, 1.0);
}

fn overlay_with_opacity(dst: &mut RgbaFrame, src: &RgbaFrame, x: i64, y: i64, opacity: f64) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }
    for sy in 0..src.height as i64 {
        let dy = y + sy;
        if dy < 0 || dy >= dst.height as i64 {
            continue;
        }
        for sx in 0..src.width as i64 {
            let dx = x + sx;
            if dx < 0 || dx >= dst.width as i64 {
                continue;
            }
            let mut px = src.get_pixel(sx as u32, sy as u32);
            if px[3] == 0 {
                continue;
            }
            px[3] = ((px[3] as f64) * opacity).round().clamp(0.0, 255.0) as u8;
            let dst_px = dst.get_pixel(dx as u32, dy as u32);
            dst.set_pixel(dx as u32, dy as u32, alpha_blend(dst_px, px));
        }
    }
}

fn alpha_blend(dst: [u8; 4], src: [u8; 4]) -> [u8; 4] {
    let src_a = src[3] as f64 / 255.0;
    let dst_a = dst[3] as f64 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= 0.0 {
        return [0, 0, 0, 0];
    }

    let mut out = [0u8; 4];
    for c in 0..3 {
        let src_c = src[c] as f64 / 255.0;
        let dst_c = dst[c] as f64 / 255.0;
        let out_c = (src_c * src_a + dst_c * dst_a * (1.0 - src_a)) / out_a;
        out[c] = (out_c * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    out[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
    out
}

fn draw_rounded_rect(
    dst: &mut RgbaFrame,
    x: i64,
    y: i64,
    w: u32,
    h: u32,
    radius: f64,
    color: [u8; 4],
) {
    for py in 0..h as i64 {
        for px in 0..w as i64 {
            let dx = x + px;
            let dy = y + py;
            if dx < 0 || dy < 0 || dx >= dst.width as i64 || dy >= dst.height as i64 {
                continue;
            }
            if !point_in_rounded_rect(px as f64 + 0.5, py as f64 + 0.5, w as f64, h as f64, radius)
            {
                continue;
            }
            let dst_px = dst.get_pixel(dx as u32, dy as u32);
            dst.set_pixel(dx as u32, dy as u32, alpha_blend(dst_px, color));
        }
    }
}

fn draw_rounded_rect_border(
    dst: &mut RgbaFrame,
    x: i64,
    y: i64,
    w: u32,
    h: u32,
    radius: f64,
    thickness: u32,
    color: [u8; 4],
) {
    let t = thickness as f64;
    let inner_w = (w as f64 - 2.0 * t).max(0.0);
    let inner_h = (h as f64 - 2.0 * t).max(0.0);
    let inner_radius = (radius - t).max(0.0);

    for py in 0..h as i64 {
        for px in 0..w as i64 {
            let dx = x + px;
            let dy = y + py;
            if dx < 0 || dy < 0 || dx >= dst.width as i64 || dy >= dst.height as i64 {
                continue;
            }
            let in_outer =
                point_in_rounded_rect(px as f64 + 0.5, py as f64 + 0.5, w as f64, h as f64, radius);
            if !in_outer {
                continue;
            }
            let in_inner = if inner_w > 0.0 && inner_h > 0.0 {
                point_in_rounded_rect(
                    px as f64 + 0.5 - t,
                    py as f64 + 0.5 - t,
                    inner_w,
                    inner_h,
                    inner_radius,
                )
            } else {
                false
            };
            if in_inner {
                continue;
            }
            let dst_px = dst.get_pixel(dx as u32, dy as u32);
            dst.set_pixel(dx as u32, dy as u32, alpha_blend(dst_px, color));
        }
    }
}

fn point_in_rounded_rect(px: f64, py: f64, w: f64, h: f64, radius: f64) -> bool {
    if px < 0.0 || py < 0.0 || px > w || py > h {
        return false;
    }
    let r = radius.max(0.0);
    let x = px;
    let y = py;

    let left = r;
    let right = w - r;
    let top = r;
    let bottom = h - r;

    if x >= left && x <= right && y >= 0.0 && y <= h {
        return true;
    }
    if y >= top && y <= bottom && x >= 0.0 && x <= w {
        return true;
    }

    let cx = if x < left { left } else { right };
    let cy = if y < top { top } else { bottom };
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= r * r
}

fn draw_rounded_rect_mask(
    mask: &mut [u8],
    mask_w: u32,
    mask_h: u32,
    x: i64,
    y: i64,
    w: u32,
    h: u32,
    radius: f64,
) {
    for py in 0..h as i64 {
        for px in 0..w as i64 {
            let dx = x + px;
            let dy = y + py;
            if dx < 0 || dy < 0 || dx >= mask_w as i64 || dy >= mask_h as i64 {
                continue;
            }
            if !point_in_rounded_rect(px as f64 + 0.5, py as f64 + 0.5, w as f64, h as f64, radius)
            {
                continue;
            }
            mask[(dy as u32 * mask_w + dx as u32) as usize] = 255;
        }
    }
}

fn blur_mask_box(mask: &mut [u8], width: usize, height: usize, radius: usize, passes: usize) {
    if radius == 0 || width == 0 || height == 0 {
        return;
    }

    let mut tmp = vec![0u8; width * height];
    for _ in 0..passes.max(1) {
        // Horizontal
        for y in 0..height {
            let row = &mask[y * width..(y + 1) * width];
            let out_row = &mut tmp[y * width..(y + 1) * width];
            box_blur_1d(row, out_row, radius);
        }
        // Vertical
        for x in 0..width {
            let mut col = vec![0u8; height];
            for y in 0..height {
                col[y] = tmp[y * width + x];
            }
            let mut out_col = vec![0u8; height];
            box_blur_1d(&col, &mut out_col, radius);
            for y in 0..height {
                mask[y * width + x] = out_col[y];
            }
        }
    }
}

fn box_blur_1d(input: &[u8], output: &mut [u8], radius: usize) {
    let n = input.len();
    if n == 0 {
        return;
    }
    let r = radius.min(n.saturating_sub(1));
    let window = r * 2 + 1;
    let mut padded = Vec::with_capacity(n + 2 * r);
    padded.extend(std::iter::repeat(input[0]).take(r));
    padded.extend_from_slice(input);
    padded.extend(std::iter::repeat(input[n - 1]).take(r));

    let mut sum: u32 = padded[..window].iter().map(|&v| v as u32).sum();
    for i in 0..n {
        output[i] = ((sum as f64 / window as f64).round().clamp(0.0, 255.0)) as u8;
        if i + window < padded.len() {
            sum = sum
                .saturating_sub(padded[i] as u32)
                .saturating_add(padded[i + window] as u32);
        }
    }
}

fn apply_shadow_supersampled(
    dst: &mut RgbaFrame,
    mask: &[u8],
    mask_width: usize,
    mask_height: usize,
    supersample: usize,
    rgb: [u8; 3],
    max_alpha: u8,
) {
    if supersample == 0 || mask_width == 0 || mask_height == 0 {
        return;
    }
    if mask.len() < mask_width.saturating_mul(mask_height) {
        return;
    }

    for y in 0..dst.height as usize {
        for x in 0..dst.width as usize {
            let start_x = x.saturating_mul(supersample);
            let start_y = y.saturating_mul(supersample);
            if start_x >= mask_width || start_y >= mask_height {
                continue;
            }
            let end_x = (start_x + supersample).min(mask_width);
            let end_y = (start_y + supersample).min(mask_height);

            let mut sum = 0u32;
            let mut count = 0u32;
            for sy in start_y..end_y {
                let row_base = sy.saturating_mul(mask_width);
                for sx in start_x..end_x {
                    sum = sum.saturating_add(mask[row_base + sx] as u32);
                    count = count.saturating_add(1);
                }
            }
            if count == 0 {
                continue;
            }

            let avg = ((sum as f64) / (count as f64)).round().clamp(0.0, 255.0) as u8;
            if avg == 0 {
                continue;
            }

            let a = ((avg as f64 / 255.0) * max_alpha as f64)
                .round()
                .clamp(0.0, 255.0) as u8;
            if a == 0 {
                continue;
            }

            let px_idx = (y * dst.width as usize + x) * 4;
            if px_idx + 3 >= dst.data.len() {
                continue;
            }
            let dst_px = [
                dst.data[px_idx],
                dst.data[px_idx + 1],
                dst.data[px_idx + 2],
                dst.data[px_idx + 3],
            ];
            let out_px = alpha_blend(dst_px, [rgb[0], rgb[1], rgb[2], a]);
            dst.data[px_idx] = out_px[0];
            dst.data[px_idx + 1] = out_px[1];
            dst.data[px_idx + 2] = out_px[2];
            dst.data[px_idx + 3] = out_px[3];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn format_key_display_replaces_common_keys() {
        assert_eq!(format_key_display("Enter").as_deref(), Some("⏎"));
        assert_eq!(format_key_display("Cmd+S").as_deref(), Some("⌘+S"));
        assert_eq!(format_key_display("space").as_deref(), Some("␣"));
    }

    #[test]
    fn sanitize_display_text_preserves_action_key_in_modifier_combo() {
        let bytes = include_bytes!(
            "../../../../../../../vscode/src/vs/base/browser/ui/codicons/codicon/codicon.ttf"
        );
        let font = Font::from_bytes(bytes.as_slice(), FontSettings::default())
            .expect("codicon.ttf should parse");

        assert!(!font_has_glyph(&font, '⌘'));
        assert!(!font_has_glyph(&font, '⏎'));

        let out = sanitize_display_text_for_font("⌘+⏎", &font);
        assert_eq!(out, "Cmd+Enter");
    }

    #[test]
    fn timeline_selects_active_event() {
        let events = vec![KeystrokeEvent {
            video_timestamp_ms: 1000.0,
            display_text: "Cmd".into(),
            event_type: KeystrokeEventType::KeySingle,
            display_duration_ms: 0.0,
            action_index: 0,
        }];
        let tl = KeystrokeTimeline::new(&events);
        assert!(tl.state_at(0.0).is_none());
        assert!(tl.state_at(900.0).is_some()); // show start at 850ms
    }

    #[test]
    fn box_blur_1d_handles_single_element() {
        let input = [255u8];
        let mut out = [0u8];
        box_blur_1d(&input, &mut out, 8);
        assert_eq!(out[0], 255);
    }

    #[test]
    fn keystroke_shadow_does_not_clip_at_frame_edges() {
        let bytes = include_bytes!(
            "../../../../../../../vscode/src/vs/base/browser/ui/codicons/codicon/codicon.ttf"
        );
        let font = Font::from_bytes(bytes.as_slice(), FontSettings::default())
            .expect("codicon.ttf should parse");

        let overlay = render_keystroke_overlay("S", 1.0, &font).expect("overlay should render");
        let frame = &overlay.frame;
        let mut edge_alpha = 0u8;

        for x in 0..frame.width {
            edge_alpha = edge_alpha.max(frame.get_pixel(x, 0)[3]);
            edge_alpha = edge_alpha.max(frame.get_pixel(x, frame.height - 1)[3]);
        }
        for y in 0..frame.height {
            edge_alpha = edge_alpha.max(frame.get_pixel(0, y)[3]);
            edge_alpha = edge_alpha.max(frame.get_pixel(frame.width - 1, y)[3]);
        }

        assert_eq!(
            edge_alpha, 0,
            "Shadow should fully fade before image edges to avoid clipping"
        );
    }

    #[test]
    fn keystroke_shadow_has_smooth_alpha_gradient() {
        let bytes = include_bytes!(
            "../../../../../../../vscode/src/vs/base/browser/ui/codicons/codicon/codicon.ttf"
        );
        let font = Font::from_bytes(bytes.as_slice(), FontSettings::default())
            .expect("codicon.ttf should parse");

        let overlay = render_keystroke_overlay("S", 1.0, &font).expect("overlay should render");
        let frame = &overlay.frame;
        let pill_x0 = overlay.content_offset_x.max(0) as u32;
        let pill_y0 = overlay.content_offset_y.max(0) as u32;
        let pill_x1 = pill_x0.saturating_add(overlay.pill_width);
        let pill_y1 = pill_y0.saturating_add(overlay.pill_height);

        let mut shadow_alphas = BTreeSet::new();
        for y in 0..frame.height {
            for x in 0..frame.width {
                if x >= pill_x0 && x < pill_x1 && y >= pill_y0 && y < pill_y1 {
                    continue;
                }
                let alpha = frame.get_pixel(x, y)[3];
                if alpha > 0 {
                    shadow_alphas.insert(alpha);
                }
            }
        }

        assert!(
            shadow_alphas.len() >= 16,
            "Expected a smooth shadow gradient with many alpha levels, found {}",
            shadow_alphas.len()
        );
    }

    #[test]
    fn keystroke_border_is_visibly_lighter_than_fill() {
        let bytes = include_bytes!(
            "../../../../../../../vscode/src/vs/base/browser/ui/codicons/codicon/codicon.ttf"
        );
        let font = Font::from_bytes(bytes.as_slice(), FontSettings::default())
            .expect("codicon.ttf should parse");

        let overlay = render_keystroke_overlay("S", 1.0, &font).expect("overlay should render");
        let frame = &overlay.frame;

        let pill_x = overlay.content_offset_x.max(0) as u32;
        let pill_y = overlay.content_offset_y.max(0) as u32;
        let border_x = pill_x + overlay.pill_width / 2;
        let border_y = pill_y;
        let fill_x = border_x;
        let fill_y = pill_y + overlay.pill_height / 2;

        let border_px = frame.get_pixel(border_x, border_y);
        let fill_px = frame.get_pixel(fill_x, fill_y);
        let border_luma = border_px[0] as i16;
        let fill_luma = fill_px[0] as i16;

        assert!(
            border_luma >= fill_luma + 8,
            "Expected border to be visibly lighter than fill (border={}, fill={})",
            border_luma,
            fill_luma
        );
    }
}
