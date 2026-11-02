//! Draws notifications as images, on the processor: no web page and no graphics card time from
//! Encore. Each notification becomes a picture of the card (or edge tab) once, from a small SVG
//! drawn by resvg; every frame those pictures are placed in a stack and the timer lines drawn on
//! top. The window that shows the result is in notify.rs.
//!
//! Sizes here are in logical pixels (as at 100 % display scale); `scale` turns them into the
//! screen's pixels.

#![cfg_attr(not(windows), allow(dead_code))]

use resvg::tiny_skia::{self, Pixmap, PixmapPaint, Rect, Transform};
use resvg::usvg;
use std::sync::{Arc, OnceLock};

/// The notification window's size for cards and for edge tabs (logical pixels).
pub const CARD_WINDOW_W: f32 = 400.0;
pub const TAB_WINDOW_W: f32 = 340.0;
pub const WINDOW_H: f32 = 320.0;

/// Room around each card for its glow.
const GLOW: f32 = 20.0;
/// Between stacked cards.
const GAP: f32 = 10.0;
const CARD_W: f32 = 360.0;
const TAB_W: f32 = 320.0;
/// Fading in and out, in seconds.
const IN: f32 = 0.22;
const OUT: f32 = 0.26;
/// How long a card takes to move to its new place when the stack changes.
const SLIDE: f32 = 0.2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Echo,
    Record,
    Saved,
    Game,
    Problem,
}

impl Kind {
    pub fn parse(s: &str) -> Kind {
        match s {
            "echo" => Kind::Echo,
            "record" => Kind::Record,
            "saved" => Kind::Saved,
            "game" => Kind::Game,
            _ => Kind::Problem,
        }
    }
    /// (lines and icon, border, glow): the same colours as the interface.
    fn colours(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Kind::Echo => ("#b4bbff", "rgba(138,150,255,0.55)", "rgba(88,101,242,0.45)"),
            Kind::Record => ("#ff8a8e", "rgba(255,120,124,0.55)", "rgba(242,63,67,0.42)"),
            Kind::Saved => ("#6fdc8c", "rgba(111,220,140,0.5)", "rgba(35,165,90,0.42)"),
            Kind::Game => ("#c3b4ff", "rgba(170,140,255,0.5)", "rgba(130,100,255,0.42)"),
            Kind::Problem => ("#ffd27a", "rgba(240,178,50,0.55)", "rgba(240,178,50,0.36)"),
        }
    }
    fn rgb(self) -> (u8, u8, u8) {
        match self {
            Kind::Echo => (0xb4, 0xbb, 0xff),
            Kind::Record => (0xff, 0x8a, 0x8e),
            Kind::Saved => (0x6f, 0xdc, 0x8c),
            Kind::Game => (0xc3, 0xb4, 0xff),
            Kind::Problem => (0xff, 0xd2, 0x7a),
        }
    }
    /// The small heading on edge tabs.
    fn heading(self) -> &'static str {
        match self {
            Kind::Echo => "Echoes",
            Kind::Record | Kind::Saved => "Recording",
            Kind::Game => "Game detection",
            Kind::Problem => "Encore",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Kind::Echo => r#"<path d="M3 12a9 9 0 1 0 2.6-6.4L3 8"/><path d="M3 3v5h5"/><path d="M12 8v4l3 2"/>"#,
            Kind::Record => r#"<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="3.5" fill="currentColor"/>"#,
            Kind::Saved => r#"<path d="M20 6 9 17l-5-5"/>"#,
            Kind::Game => r#"<path d="M7 9h10a4 4 0 0 1 4 4v1.5a2.5 2.5 0 0 1-4.5 1.5L15 14H9l-1.5 2A2.5 2.5 0 0 1 3 14.5V13a4 4 0 0 1 4-4z"/><path d="M8 11.5v2M7 12.5h2M15.5 12h.01M17.5 13h.01"/>"#,
            Kind::Problem => r#"<path d="M12 3.5 2.5 20h19L12 3.5z"/><path d="M12 10v4.5M12 17.2h.01"/>"#,
        }
    }
}

/// Where the notifications sit: the spot's edge decides the look and the stacking.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spot {
    Top,
    Bottom,
    Left,
    Right,
}

impl Spot {
    pub fn parse(pos: &str) -> Spot {
        match pos {
            "ml" => Spot::Left,
            "mr" => Spot::Right,
            p if p.starts_with('b') => Spot::Bottom,
            _ => Spot::Top,
        }
    }
    pub fn is_tab(self) -> bool {
        matches!(self, Spot::Left | Spot::Right)
    }
    pub fn window_w(self) -> f32 {
        if self.is_tab() { TAB_WINDOW_W } else { CARD_WINDOW_W }
    }
}

// ---------- fonts and text ----------

fn options() -> &'static usvg::Options<'static> {
    static OPT: OnceLock<usvg::Options<'static>> = OnceLock::new();
    OPT.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        // The interface's font; something close where it's missing (tests on other systems).
        db.set_sans_serif_family("Segoe UI");
        usvg::Options { fontdb: Arc::new(db), font_family: "Segoe UI".into(), ..Default::default() }
    })
}

/// Loads the fonts now (it takes a moment), so the first notification isn't late.
pub fn warm_up() {
    let _ = options();
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const FONT: &str = "Segoe UI, Segoe UI Variable Text, Arial, sans-serif";

/// How wide a line of text is, in logical pixels.
fn text_width(text: &str, size: f32, weight: u16) -> f32 {
    if text.trim().is_empty() {
        return 0.0;
    }
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2000" height="60"><text x="0" y="40" font-family="{FONT}" font-size="{size}" font-weight="{weight}">{}</text></svg>"#,
        esc(text)
    );
    usvg::Tree::from_str(&svg, options()).map(|t| t.root().abs_bounding_box().width()).unwrap_or(text.len() as f32 * size * 0.5)
}

/// Fits text into at most `lines` lines of `width`, breaking between words and ending with "…"
/// if it doesn't all fit.
fn wrap(text: &str, size: f32, weight: u16, width: f32, lines: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    while i < words.len() {
        let candidate = if line.is_empty() { words[i].to_string() } else { format!("{line} {}", words[i]) };
        if text_width(&candidate, size, weight) <= width || line.is_empty() {
            line = candidate;
            i += 1;
        } else {
            out.push(std::mem::take(&mut line));
            if out.len() == lines {
                break;
            }
        }
    }
    if !line.is_empty() && out.len() < lines {
        out.push(line);
    }
    // Didn't all fit: shorten the last line until it fits with "…".
    if i < words.len() || out.last().map(|l| text_width(l, size, weight) > width).unwrap_or(false) {
        if let Some(last) = out.last_mut() {
            let mut chars: Vec<char> = last.chars().collect();
            loop {
                let s: String = chars.iter().collect::<String>().trim_end().to_string() + "…";
                if text_width(&s, size, weight) <= width || chars.is_empty() {
                    *last = s;
                    break;
                }
                chars.pop();
            }
        }
    }
    out
}

// ---------- one notification as a picture ----------

/// A notification drawn once: its picture (card and glow) and the card's place in it.
pub struct Sprite {
    pub pixmap: Pixmap,
    /// Logical height of the picture.
    pub h: f32,
    /// The card inside the picture (logical).
    pub card_x: f32,
    pub card_y: f32,
    pub card_w: f32,
    pub card_h: f32,
}

/// The edge tab's timer: thickness, and where it runs (x, top, bottom), inside the rounded edge.
const TAB_TIMER_W: f32 = 3.0;
fn tab_timer(spot: Spot, x0: f32, x1: f32, y0: f32, y1: f32) -> (f32, f32, f32) {
    let inset = 5.0 + TAB_TIMER_W / 2.0;
    let x = if spot == Spot::Right { x0 + inset } else { x1 - inset };
    (x, y0 + 9.0, y1 - 9.0)
}

pub fn draw_item(kind: Kind, title: &str, text: &str, spot: Spot, scale: f32) -> Option<Sprite> {
    let (line, border, glow) = kind.colours();
    let tab = spot.is_tab();
    let card_w = if tab { TAB_W } else { CARD_W };
    // Text: the card has an icon; the tab has a heading instead.
    let text_x = if tab { if spot == Spot::Right { 20.0 } else { 16.0 } } else { 14.0 + 38.0 + 12.0 };
    let text_w = card_w - text_x - 16.0;
    let title_lines = wrap(title, 14.0, 600, text_w, 1);
    let text_lines = if text.trim().is_empty() { Vec::new() } else { wrap(text, 12.5, 400, text_w, 2) };
    let heading_h = if tab { 16.0 } else { 0.0 };
    let body_h = heading_h + 19.0 + text_lines.len() as f32 * 17.0;
    let card_h = if tab { 12.0 + body_h + 12.0 } else { (12.0 + body_h.max(38.0) + 12.0).max(62.0) };
    let w = card_w + if tab { GLOW } else { 2.0 * GLOW };
    let h = card_h + 2.0 * GLOW;
    let card_x = if spot == Spot::Right || !tab { GLOW } else { 0.0 };
    let card_y = GLOW;

    // The card's outline: rounded all round, or only on the side away from the screen edge.
    let r = 10.0;
    let (x0, y0, x1, y1) = (card_x, card_y, card_x + card_w, card_y + card_h);
    let shape = match spot {
        Spot::Right => format!("M{x1} {y0} H{} A{r} {r} 0 0 0 {x0} {} V{} A{r} {r} 0 0 0 {} {y1} H{x1} Z", x0 + r, y0 + r, y1 - r, x0 + r),
        Spot::Left => format!("M{x0} {y0} H{} A{r} {r} 0 0 1 {x1} {} V{} A{r} {r} 0 0 1 {} {y1} H{x0} Z", x1 - r, y0 + r, y1 - r, x1 - r),
        _ => format!(
            "M{} {y0} H{} A{r} {r} 0 0 1 {x1} {} V{} A{r} {r} 0 0 1 {} {y1} H{} A{r} {r} 0 0 1 {x0} {} V{} A{r} {r} 0 0 1 {} {y0} Z",
            x0 + r, x1 - r, y0 + r, y1 - r, x1 - r, x0 + r, y1 - r, y0 + r, x0 + r
        ),
    };
    // The border: the whole outline for cards; for tabs, not along the screen edge.
    let border_path = match spot {
        Spot::Right => format!("M{x1} {y0} H{} A{r} {r} 0 0 0 {x0} {} V{} A{r} {r} 0 0 0 {} {y1} H{x1}", x0 + r, y0 + r, y1 - r, x0 + r),
        Spot::Left => format!("M{x0} {y0} H{} A{r} {r} 0 0 1 {x1} {} V{} A{r} {r} 0 0 1 {} {y1} H{x0}", x1 - r, y0 + r, y1 - r, x1 - r),
        _ => shape.clone(),
    };

    let mut body = String::new();
    // Glow behind, then the glassy card, then its border.
    body += &format!(r#"<path d="{shape}" fill="{glow}" filter="url(#glow)"/>"#);
    body += &format!(r#"<path d="{shape}" fill="rgba(16,15,26,0.9)"/>"#);
    body += &format!(r#"<path d="{border_path}" fill="none" stroke="{border}" stroke-width="1"/>"#);
    let mut ty = card_y + 12.0;
    if tab {
        // The timer's rail on the inner side, inside the rounded edge (the timer itself is drawn
        // each frame, in the same place).
        let (rx, ry0, ry1) = tab_timer(spot, x0, x1, y0, y1);
        body += &format!(
            r#"<line x1="{rx}" y1="{ry0}" x2="{rx}" y2="{ry1}" stroke="rgba(255,255,255,0.07)" stroke-width="{TAB_TIMER_W}" stroke-linecap="round"/>"#
        );
        body += &format!(
            r#"<text x="{}" y="{}" font-family="{FONT}" font-size="11.5" font-weight="600" fill="{line}">{}</text>"#,
            card_x + text_x,
            ty + 11.5,
            esc(kind.heading())
        );
        ty += heading_h;
    } else {
        // The icon in its box.
        let ix = card_x + 14.0;
        let iy = card_y + (card_h - 38.0) / 2.0;
        body += &format!(r#"<rect x="{}" y="{}" width="37" height="37" rx="8.5" fill="none" stroke="{border}"/>"#, ix + 0.5, iy + 0.5);
        let icon = kind.icon().replace("currentColor", line);
        let place = format!("translate({} {}) scale({})", ix + 8.0, iy + 8.0, 22.0 / 24.0);
        body += &format!(r#"<g transform="{place}" fill="none" stroke="{line}" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" filter="url(#soft)" opacity="0.8">{icon}</g>"#);
        body += &format!(r#"<g transform="{place}" fill="none" stroke="{line}" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">{icon}</g>"#);
        ty = card_y + (card_h - body_h) / 2.0;
    }
    for t in &title_lines {
        body += &format!(
            r##"<text x="{}" y="{}" font-family="{FONT}" font-size="14" font-weight="600" fill="#ffffff">{}</text>"##,
            card_x + text_x,
            ty + 14.5,
            esc(t)
        );
        ty += 19.0;
    }
    for t in &text_lines {
        body += &format!(
            r##"<text x="{}" y="{}" font-family="{FONT}" font-size="12.5" fill="#b3b2bd">{}</text>"##,
            card_x + text_x,
            ty + 12.5,
            esc(t)
        );
        ty += 17.0;
    }
    let (pw, ph) = ((w * scale).ceil() as u32, (h * scale).ceil() as u32);
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{pw}" height="{ph}" viewBox="0 0 {w} {h}">
<defs>
<filter id="glow" filterUnits="userSpaceOnUse" x="-40" y="-40" width="{}" height="{}"><feGaussianBlur stdDeviation="9"/></filter>
<filter id="soft" filterUnits="userSpaceOnUse" x="-40" y="-40" width="{}" height="{}"><feGaussianBlur stdDeviation="1.4"/></filter>
</defs>{body}</svg>"#,
        w + 80.0,
        h + 80.0,
        w + 80.0,
        h + 80.0
    );
    let tree = usvg::Tree::from_str(&svg, options()).ok()?;
    let mut pixmap = Pixmap::new(pw.max(1), ph.max(1))?;
    resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());
    Some(Sprite { pixmap, h, card_x, card_y, card_w, card_h })
}

// ---------- the stack, frame by frame ----------

pub struct Shown {
    pub kind: Kind,
    pub sprite: Sprite,
    pub seconds: f32,
    /// Notifications with the same id replace each other in place ("Saving an Echo…" turning
    /// into "Echo saved").
    pub id: Option<String>,
    /// Whether the timer line shows (not while something is still being saved).
    pub timer: bool,
    /// When the timer started (a replacement starts it again).
    timer_from: f32,
    /// When it arrived and (if going) when it started to go, in seconds on the stack's clock.
    pub born: f32,
    pub leaving: Option<f32>,
    /// Its place in the window (logical y): moving from `from_y` to `to_y` since `moved_at`.
    y: Option<f32>,
    from_y: f32,
    to_y: f32,
    moved_at: f32,
}

impl Shown {
    pub fn new(kind: Kind, sprite: Sprite, seconds: f32, now: f32) -> Shown {
        Shown { kind, sprite, seconds, id: None, timer: true, timer_from: now, born: now, leaving: None, y: None, from_y: 0.0, to_y: 0.0, moved_at: now }
    }

    pub fn with_id(mut self, id: Option<String>, timer: bool) -> Shown {
        self.id = id;
        self.timer = timer;
        self
    }
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Up to three notifications at one spot. Newest last.
pub struct Stack {
    pub spot: Spot,
    pub items: Vec<Shown>,
    pub scale: f32,
}

pub const MAX: usize = 3;

impl Stack {
    pub fn new(spot: Spot, scale: f32) -> Stack {
        Stack { spot, items: Vec::new(), scale }
    }

    pub fn add(&mut self, item: Shown, now: f32) {
        // Same id as one still showing: it becomes the new one, where it is.
        if let Some(id) = item.id.as_ref() {
            if let Some(old) = self.items.iter_mut().find(|i| i.leaving.is_none() && i.id.as_ref() == Some(id)) {
                old.kind = item.kind;
                old.sprite = item.sprite;
                old.seconds = item.seconds;
                old.timer = item.timer;
                old.timer_from = now;
                return;
            }
        }
        // A fourth pushes the oldest still showing out.
        while self.items.iter().filter(|i| i.leaving.is_none()).count() >= MAX {
            if let Some(old) = self.items.iter_mut().find(|i| i.leaving.is_none()) {
                old.leaving = Some(now);
            }
        }
        self.items.push(item);
    }

    /// Starts the fade of any whose time is up, and drops those that have faded.
    pub fn update(&mut self, now: f32) {
        for i in &mut self.items {
            if i.leaving.is_none() && now - i.timer_from >= i.seconds {
                i.leaving = Some(now);
            }
        }
        self.items.retain(|i| i.leaving.map(|l| now - l < OUT).unwrap_or(true));
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Where each (staying) notification belongs: newest nearest the screen edge.
    fn targets(&self) -> Vec<f32> {
        let staying: Vec<usize> = (0..self.items.len()).filter(|&k| self.items[k].leaving.is_none()).collect();
        let mut out = vec![0.0; self.items.len()];
        let heights: Vec<f32> = staying.iter().map(|&k| self.items[k].sprite.h).collect();
        let total: f32 = heights.iter().sum::<f32>() - 2.0 * GLOW * heights.len().saturating_sub(1) as f32
            + GAP * heights.len().saturating_sub(1) as f32;
        match self.spot {
            Spot::Bottom => {
                // Newest at the bottom.
                let mut bottom = WINDOW_H;
                for &k in staying.iter().rev() {
                    let h = self.items[k].sprite.h;
                    out[k] = bottom - h;
                    bottom = bottom - h + 2.0 * GLOW - GAP;
                }
            }
            Spot::Top => {
                // Newest at the top.
                let mut top = 0.0;
                for &k in staying.iter().rev() {
                    out[k] = top;
                    top += self.items[k].sprite.h - 2.0 * GLOW + GAP;
                }
            }
            Spot::Left | Spot::Right => {
                let mut top = (WINDOW_H - total) / 2.0;
                for &k in staying.iter().rev() {
                    out[k] = top;
                    top += self.items[k].sprite.h - 2.0 * GLOW + GAP;
                }
            }
        }
        out
    }

    /// The window's picture at `now` (screen pixels, premultiplied RGBA).
    pub fn frame(&mut self, now: f32) -> Option<Pixmap> {
        let s = self.scale;
        let (ww, wh) = ((self.spot.window_w() * s).round() as u32, (WINDOW_H * s).round() as u32);
        let mut canvas = Pixmap::new(ww.max(1), wh.max(1))?;
        let targets = self.targets();
        for (k, item) in self.items.iter_mut().enumerate() {
            // Moving to a new place in the stack (going ones stay where they are).
            if item.leaving.is_none() {
                let target = targets[k];
                match item.y {
                    None => {
                        item.y = Some(target);
                        item.from_y = target;
                        item.to_y = target;
                    }
                    Some(cur) => {
                        if (item.to_y - target).abs() > 0.01 {
                            item.from_y = cur;
                            item.to_y = target;
                            item.moved_at = now;
                        }
                        let t = ease((now - item.moved_at) / SLIDE);
                        item.y = Some(item.from_y + (item.to_y - item.from_y) * t);
                    }
                }
            }
            let y = item.y.unwrap_or(0.0);
            // Fade and slide in from the edge it sits against; fade out.
            let tin = ease((now - item.born) / IN);
            let alpha = match item.leaving {
                Some(l) => 1.0 - ((now - l) / OUT).clamp(0.0, 1.0),
                None => tin,
            };
            if alpha <= 0.0 {
                continue;
            }
            let (dx, dy) = match self.spot {
                Spot::Top => (0.0, -10.0 * (1.0 - tin)),
                Spot::Bottom => (0.0, 10.0 * (1.0 - tin)),
                Spot::Right => (40.0 * (1.0 - tin), 0.0),
                Spot::Left => (-40.0 * (1.0 - tin), 0.0),
            };
            let sp = &item.sprite;
            let x = dx;
            let paint = PixmapPaint { opacity: alpha, ..PixmapPaint::default() };
            canvas.draw_pixmap((x * s).round() as i32, ((y + dy) * s).round() as i32, sp.pixmap.as_ref(), &paint, Transform::identity(), None);
            if !item.timer {
                continue;
            }
            // The timer: how much of its time is left.
            let left = (1.0 - (now - item.timer_from) / item.seconds).clamp(0.0, 1.0);
            let (r, g, b) = item.kind.rgb();
            let (cx, cy) = (x + sp.card_x, y + dy + sp.card_y);
            let bars: Vec<Rect> = match self.spot {
                Spot::Top | Spot::Bottom => {
                    let bar_y = if self.spot == Spot::Top { cy + sp.card_h - 2.0 } else { cy };
                    let full = sp.card_w - 24.0;
                    Rect::from_xywh((cx + 12.0) * s, bar_y * s, (full * left).max(0.0) * s, 2.0 * s).into_iter().collect()
                }
                Spot::Right | Spot::Left => {
                    // A rounded line inside the curved edge, shrinking towards the bottom.
                    let (lx, ly0, ly1) = tab_timer(self.spot, cx, cx + sp.card_w, cy, cy + sp.card_h);
                    let top = ly1 - (ly1 - ly0) * left;
                    if ly1 - top > 0.5 {
                        let mut pb = tiny_skia::PathBuilder::new();
                        pb.move_to(lx * s, top * s);
                        pb.line_to(lx * s, ly1 * s);
                        if let Some(path) = pb.finish() {
                            let mut paint = tiny_skia::Paint { anti_alias: true, ..Default::default() };
                            for (grow, a) in [(6.0, 0.10), (3.0, 0.20), (0.0, 1.0)] {
                                paint.set_color_rgba8(r, g, b, (a * alpha * 255.0) as u8);
                                let stroke = tiny_skia::Stroke { width: (TAB_TIMER_W + grow) * s, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
                                canvas.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                            }
                        }
                    }
                    Vec::new()
                }
            };
            for bar in bars {
                // A soft glow, then the line.
                let mut paint = tiny_skia::Paint { anti_alias: true, ..Default::default() };
                for (grow, a) in [(3.0, 0.10), (1.5, 0.18)] {
                    if let Some(gl) = Rect::from_ltrb(bar.left() - grow * s, bar.top() - grow * s, bar.right() + grow * s, bar.bottom() + grow * s) {
                        paint.set_color_rgba8(r, g, b, (a * alpha * 255.0) as u8);
                        canvas.fill_rect(gl, &paint, Transform::identity(), None);
                    }
                }
                paint.set_color_rgba8(r, g, b, (alpha * 255.0) as u8);
                canvas.fill_rect(bar, &paint, Transform::identity(), None);
            }
        }
        Some(canvas)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack_png(spot: Spot, name: &str) {
        let mut st = Stack::new(spot, 1.0);
        let items = [
            (Kind::Echo, "Echo saved", "Last 30 seconds of Warframe"),
            (Kind::Game, "Now following Warframe", "Press Ctrl+Alt+G to stay on this game"),
            (Kind::Problem, "Recording is falling behind", "Warframe is using all of your graphics card, so Encore can't keep up. Capping the game's frame rate a little below your monitor's usually fixes it."),
        ];
        for (i, (k, t, x)) in items.iter().enumerate() {
            let sp = draw_item(*k, t, x, spot, 1.0).unwrap();
            st.add(Shown::new(*k, sp, 5.0, i as f32 * 0.05), i as f32 * 0.05);
        }
        let mut f = st.frame(1.0).unwrap();
        // Something to see it against.
        let mut bg = Pixmap::new(f.width(), f.height()).unwrap();
        bg.fill(tiny_skia::Color::from_rgba8(0x2b, 0x33, 0x40, 255));
        bg.draw_pixmap(0, 0, f.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
        f = bg;
        if let Ok(dir) = std::env::var("ENCORE_NOTIFY_PNG") {
            f.save_png(format!("{dir}/{name}.png")).unwrap();
        }
    }

    #[test]
    fn draws_each_spot() {
        stack_png(Spot::Top, "top");
        stack_png(Spot::Bottom, "bottom");
        stack_png(Spot::Right, "right");
        stack_png(Spot::Left, "left");
    }

    #[test]
    fn long_text_wraps_to_two_lines_with_an_ellipsis() {
        let lines = wrap("one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen", 12.5, 400, 150.0, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with('…'));
        assert_eq!(wrap("Short", 14.0, 600, 200.0, 1), vec!["Short".to_string()]);
    }

    #[test]
    fn the_stack_keeps_three_and_lets_them_go() {
        let mut st = Stack::new(Spot::Top, 1.0);
        for i in 0..4 {
            let sp = draw_item(Kind::Echo, "Echo saved", "", Spot::Top, 1.0).unwrap();
            st.add(Shown::new(Kind::Echo, sp, 5.0, i as f32), i as f32);
        }
        assert_eq!(st.items.iter().filter(|i| i.leaving.is_none()).count(), 3);
        st.update(3.0 + OUT + 0.01);
        assert_eq!(st.items.len(), 3); // the pushed-out one has faded
        st.update(20.0);
        st.update(20.0 + OUT + 0.01);
        assert!(st.is_empty());
    }

    #[test]
    fn a_saving_card_turns_into_the_saved_one_in_place() {
        let mut st = Stack::new(Spot::Top, 1.0);
        let saving = draw_item(Kind::Echo, "Saving an Echo…", "The last minute", Spot::Top, 1.0).unwrap();
        st.add(Shown::new(Kind::Echo, saving, 30.0, 0.0).with_id(Some("e1".into()), false), 0.0);
        let other = draw_item(Kind::Game, "Now following Warframe", "", Spot::Top, 1.0).unwrap();
        st.add(Shown::new(Kind::Game, other, 5.0, 1.0), 1.0);
        let saved = draw_item(Kind::Echo, "Echo saved", "Last 60 seconds", Spot::Top, 1.0).unwrap();
        st.add(Shown::new(Kind::Echo, saved, 5.0, 4.0).with_id(Some("e1".into()), true), 4.0);
        assert_eq!(st.items.len(), 2); // replaced, not added
        assert!(st.items[0].timer && st.items[0].id.as_deref() == Some("e1"));
        // Its timer started again when it was replaced.
        st.update(8.0);
        assert!(st.items[0].leaving.is_none());
        st.update(9.1);
        assert!(st.items[0].leaving.is_some());
    }
}
