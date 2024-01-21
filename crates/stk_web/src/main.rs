use std::borrow::Cow;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use gloo::events::EventListener;
use gloo::render::{request_animation_frame, AnimationFrame};
use gloo::utils::document;
use js_sys::wasm_bindgen::JsValue;
use ordered_float::NotNan;
use tracing_subscriber::fmt::format::Pretty;
use tracing_subscriber::prelude::*;
use tracing_web::{performance_layer, MakeWebConsoleWriter};
use wasm_bindgen_futures::spawn_local;
use web_sys::wasm_bindgen::closure::Closure;
use web_sys::wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, Element, Event, HtmlCanvasElement, HtmlElement, MouseEvent,
    ResizeObserverEntry,
};

fn main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .without_time() // std::time is not available on browsers
        .with_writer(MakeWebConsoleWriter::new());
    let perf_layer = performance_layer().with_details_from_fields(Pretty::default());
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(perf_layer)
        .init();

    spawn_local(run());
}

async fn run() {
    let canvas = document().get_element_by_id("main").unwrap();
    let canvas: HtmlCanvasElement = canvas.dyn_into().unwrap();

    RenderLoop::new(canvas).run().await;
}

struct ResizeObserver {
    ws: web_sys::ResizeObserver,
    #[allow(dead_code)]
    closure: Closure<dyn FnMut(js_sys::Object)>,
}
impl ResizeObserver {
    fn new(mut f: impl FnMut(&[ResizeObserverEntry]) + 'static) -> Self {
        let closure = Closure::new(move |obj: js_sys::Object| {
            let entries: js_sys::Array = obj.dyn_into().unwrap();
            let converted = entries
                .into_iter()
                .map(|x| x.dyn_into().unwrap())
                .collect::<Vec<_>>();
            f(&converted);
        });
        let ws = web_sys::ResizeObserver::new(closure.as_ref().unchecked_ref()).unwrap();
        Self { closure, ws }
    }

    fn observe(&self, target: &Element) {
        self.ws.observe(target);
    }
}
impl Drop for ResizeObserver {
    fn drop(&mut self) {
        self.ws.disconnect();
    }
}

struct RequestAnimationFrameFuture {
    raf_instance: Option<AnimationFrame>,
    ready: Rc<RefCell<Option<()>>>,
}
impl RequestAnimationFrameFuture {
    fn new() -> Self {
        Self {
            raf_instance: None,
            ready: Rc::new(RefCell::new(None)),
        }
    }
}
impl Future for RequestAnimationFrameFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match this.ready.take() {
            Some(_) => Poll::Ready(()),
            None => {
                let ready = Rc::clone(&this.ready);
                let waker = ctx.waker().to_owned();
                let instance = request_animation_frame(move |_delta| {
                    *ready.borrow_mut() = Some(());
                    waker.wake();
                });
                this.raf_instance = Some(instance);
                Poll::Pending
            }
        }
    }
}

struct RenderLoop {
    app: Rc<RefCell<App>>,
    canvas: HtmlCanvasElement,
    _resize_observer: ResizeObserver,
    event_listeners: Vec<EventListener>,
}

impl RenderLoop {
    fn listen(&mut self, event: &'static str, mut f: impl FnMut(&mut App, &Event) + 'static) {
        let ev = EventListener::new(&self.canvas, event, {
            let app = Rc::clone(&self.app);
            move |event| f(&mut app.borrow_mut(), event)
        });
        self.event_listeners.push(ev);
    }

    fn new(canvas: HtmlCanvasElement) -> Self {
        let ctx = canvas.get_context("2d").unwrap().unwrap();
        let ctx: CanvasRenderingContext2d = ctx.dyn_into().unwrap();

        let app = Rc::new(RefCell::new(App { ctx, main_scene: MainScene::new() }));

        let _resize_observer = ResizeObserver::new({
            let app = Rc::clone(&app);
            move |_entries| app.borrow_mut().on_resize()
        });
        _resize_observer.observe(&canvas);

        let mut me = Self {
            app,
            canvas,
            _resize_observer,
            event_listeners: vec![],
        };

        {
            use MouseEventType::*;
            me.listen("click", |app, ev| app.on_mouse_event(ev, Click));
            me.listen("mouseup", |app, ev| app.on_mouse_event(ev, Up));
            me.listen("mousedown", |app, ev| app.on_mouse_event(ev, Down));
            me.listen("mousemove", |app, ev| app.on_mouse_event(ev, Move));
        }

        me
    }

    async fn run(&mut self) {
        loop {
            self.app.borrow_mut().render();
            RequestAnimationFrameFuture::new().await;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum MouseEventType {
    Up,
    Down,
    Click,
    Move,
}

struct App {
    ctx: CanvasRenderingContext2d,
    main_scene: MainScene,
}

impl App {
    fn on_resize(&mut self) {
        let canvas = self.ctx.canvas().unwrap();
        let w = canvas.client_width() as u32;
        let h = canvas.client_height() as u32;
        canvas.set_width(w);
        canvas.set_height(h);
        tracing::info!("canvas resized to {w}x{h}");
        self.main_scene.render(&self.ctx);
    }

    fn mouse_event_to_pos(&self, m: &Event) -> AbsolutePos {
        let rect = self.ctx.canvas().unwrap().get_bounding_client_rect();
        let event: &MouseEvent = m.dyn_ref().unwrap();
        let x = event.client_x() as f64 - rect.left();
        let y = event.client_y() as f64 - rect.top();
        AbsolutePos { x, y }
    }

    fn on_mouse_event(&mut self, ev: &Event, ty: MouseEventType) {
        let pos = self.mouse_event_to_pos(ev);
        let pos = Renderer::new(&self.ctx).to_rel_pos(pos);
        self.main_scene.on_mouse_event(&self.ctx, pos, ty);
    }

    fn render(&mut self) {
        self.main_scene.render(&self.ctx);
    }
}

struct MainScene {
    i: usize,
    circuit: Circuit,
}

impl MainScene {
    fn new() -> Self {
        Self { i: 0, circuit: Circuit::new() }
    }

    fn renderer(&self, ctx: &CanvasRenderingContext2d) -> Renderer {
        let canvas = ctx.canvas().unwrap();
        let width = canvas.width() as f64;
        let height = canvas.height() as f64;
        let (size, offset) = {
            let (as_w, as_h) = (16.0, 9.0);
            let a = AbsoluteSize { w: width, h: width / as_w * as_h };
            let b = AbsoluteSize { w: height / as_h * as_w, h: height };
            let remain_width = a.h < height;
            if remain_width {
                (a, AbsolutePos { x: 0.0, y: (height - a.h) / 2.0 })
            } else {
                (b, AbsolutePos { x: (width - b.w) / 2.0, y: 0.0 })
            }
        };

        let ctx = Renderer::new(ctx);
        ctx.subcanbas(ctx.to_rel_rect(AbsoluteRect { pos: offset, size }))
    }

    fn on_mouse_event(&mut self, ctx: &CanvasRenderingContext2d, pos: Pos, ty: MouseEventType) {
        let pos = Renderer::new(ctx).to_abs_pos(pos); // dirty...
        let ctx = self.renderer(ctx);
        let pos = ctx.to_rel_pos(pos);
        self.circuit.on_mouse_event(&ctx, pos, ty);
    }

    fn render(&mut self, ctx: &CanvasRenderingContext2d) {
        let canvas = ctx.canvas().unwrap();
        let width = canvas.width() as f64;
        let height = canvas.height() as f64;
        ctx.set_fill_style(&JsValue::from_str("gray"));
        ctx.fill_rect(0.0, 0.0, width, height);

        let ctx = self.renderer(ctx);

        ctx.rect(Rect::FULL, Cow::from("white"), None);

        self.i += 1;

        Text {
            pos: Pos::new(0.0, 100.0),
            align: TextAlign::BottomLeft,
            text: format!("f: {}", self.i).into(),
            size: Percent::new(2.0),
        }
        .draw(&ctx);

        self.circuit.draw(&ctx);
    }
}

struct Renderer {
    // ctx.translate だと translate の translate がむずそうなのでやめた
    offset: AbsolutePos,
    /// レンダラ全体のサイズ
    size: AbsoluteSize,
    /// キャンバス全体のサイズ
    canvas_size: AbsoluteSize,
    ctx: CanvasRenderingContext2d,
}

#[derive(Debug, Clone, Copy)]
enum CursorState {
    Normal,
    Grab,
    Grabbing,
}
impl CursorState {
    fn to_css(self) -> &'static str {
        match self {
            CursorState::Normal => "default",
            CursorState::Grab => "grab",
            CursorState::Grabbing => "grabbing",
        }
    }
}

fn change_cursor_state(s: CursorState) {
    let el = document().get_element_by_id("main").unwrap();
    let el: HtmlElement = el.dyn_into().unwrap();
    el.style().set_property("cursor", s.to_css()).unwrap();
}

struct CanvasStateGuard {
    ctx: CanvasRenderingContext2d,
}
impl CanvasStateGuard {
    fn new(ctx: &CanvasRenderingContext2d) -> Self {
        let ctx = ctx.clone();
        ctx.save();
        Self { ctx }
    }
}
impl Drop for CanvasStateGuard {
    fn drop(&mut self) {
        self.ctx.restore();
    }
}

impl Renderer {
    fn new(ctx: &CanvasRenderingContext2d) -> Self {
        let canvas = ctx.canvas().unwrap();
        let size = AbsoluteSize {
            w: canvas.width() as f64,
            h: canvas.height() as f64,
        };
        let ctx = ctx.clone();
        Self {
            offset: AbsolutePos::ZERO,
            size,
            canvas_size: size,
            ctx,
        }
    }

    fn to_rel_size(&self, abs: AbsoluteSize) -> Size {
        Size {
            w: Percent::from_absolute(abs.w, self.size.w),
            h: Percent::from_absolute(abs.h, self.size.h),
        }
    }
    fn to_rel_pos(&self, abs: AbsolutePos) -> Pos {
        Pos {
            x: Percent::from_absolute(abs.x - self.offset.x, self.size.w),
            y: Percent::from_absolute(abs.y - self.offset.y, self.size.h),
        }
    }
    fn to_rel_rect(&self, abs: AbsoluteRect) -> Rect {
        Rect {
            pos: self.to_rel_pos(abs.pos),
            size: self.to_rel_size(abs.size),
        }
    }
    fn to_abs_pos(&self, rel: Pos) -> AbsolutePos {
        AbsolutePos {
            x: rel.x.to_absolute(self.size.w) + self.offset.x,
            y: rel.y.to_absolute(self.size.h) + self.offset.y,
        }
    }
    fn to_abs_size(&self, rel: Size) -> AbsoluteSize {
        AbsoluteSize {
            w: rel.w.to_absolute(self.size.w),
            h: rel.h.to_absolute(self.size.h),
        }
    }
    fn to_abs_rect(&self, rel: Rect) -> AbsoluteRect {
        AbsoluteRect {
            pos: self.to_abs_pos(rel.pos),
            size: self.to_abs_size(rel.size),
        }
    }

    fn debug(&self) {
        self.rect(
            Rect {
                pos: Pos::ZERO,
                size: Size { w: Percent::new(100.0), h: Percent::new(100.0) },
            },
            None,
            Cow::from("blue"),
        );
        self.line(
            Percent::new(0.1),
            Pos::new(0.0, 0.0),
            Pos::new(100.0, 100.0),
            Cow::from("blue"),
        );
        self.line(
            Percent::new(0.1),
            Pos::new(100.0, 0.0),
            Pos::new(0.0, 100.0),
            Cow::from("blue"),
        );
        self.set_text_align(TextAlign::TopLeft);
        self.set_font_size(Percent::new(2.0));
        self.filled_text(
            &format!(
                "{}x{}->{}x{}",
                self.canvas_size.w as u32,
                self.canvas_size.h as u32,
                self.size.w as u32,
                self.size.h as u32
            ),
            Pos::ZERO,
            "black",
        );
    }

    fn translate(&self, pos: Pos) -> Self {
        let pos = self.to_abs_pos(pos);
        Self {
            offset: pos,
            size: self.size,
            canvas_size: self.canvas_size,
            ctx: self.ctx.clone(),
        }
    }

    fn subcanbas(&self, rect: Rect) -> Self {
        let abs_rect = self.to_abs_rect(rect);
        Self {
            offset: abs_rect.pos,
            size: abs_rect.size,
            canvas_size: self.canvas_size,
            ctx: self.ctx.clone(),
        }
    }

    fn set_font_size_abs(&self, size: f64) {
        self.ctx.set_font(&format!("{size}px sans-serif"));
    }

    fn set_font_size(&self, size: Percent) {
        self.set_font_size_abs(size.to_absolute(self.size.h));
    }

    fn set_line_width(&self, width: Percent) {
        self.ctx.set_line_width(width.to_absolute(self.size.w));
    }

    fn dotted_line(&self) -> CanvasStateGuard {
        let guard = CanvasStateGuard::new(&self.ctx);
        let value = Percent::new(0.7).to_absolute(self.size.w);
        let value = JsValue::from_f64(value);
        let array = js_sys::Array::of2(&value, &value);
        self.ctx.set_line_dash(&array).unwrap();
        guard
    }

    fn set_font_to_fit(&self, text: &str, width: Percent) {
        let width = width.to_absolute(self.size.w);

        self.set_font_size_abs(1.0);
        let size = self.ctx.measure_text(text).unwrap();
        self.set_font_size_abs(width / size.width());
    }

    fn set_text_align(&self, mode: TextAlign) {
        // https://developer.mozilla.org/ja/docs/Web/API/CanvasRenderingContext2D/textAlign
        let (baseline, align) = match mode {
            TextAlign::TopLeft => ("top", "left"),
            TextAlign::Center => ("middle", "center"),
            TextAlign::BottomLeft => ("bottom", "left"),
        };
        self.ctx.set_text_baseline(baseline);
        self.ctx.set_text_align(align);
    }

    fn measure_text(&self, text: &str) -> Size {
        let measured = self.ctx.measure_text(text).unwrap();
        self.to_rel_size(AbsoluteSize {
            w: measured.width(),
            h: measured.actual_bounding_box_descent() - measured.actual_bounding_box_ascent(),
        })
    }

    fn filled_text(&self, text: &str, pos: Pos, fill_style: impl Into<Cow<'static, str>>) {
        let pos = self.to_abs_pos(pos);
        self.ctx
            .set_fill_style(&JsValue::from_str(&fill_style.into()));
        self.ctx.fill_text(text, pos.x, pos.y).unwrap();
    }

    fn rect(
        &self,
        rect: Rect,
        fill_style: impl Into<Option<Cow<'static, str>>>,
        stroke_style: impl Into<Option<Cow<'static, str>>>,
    ) {
        let rect = self.to_abs_rect(rect);

        let fill_style = fill_style.into();
        let stroke_style = stroke_style.into();

        if let Some(s) = fill_style {
            self.ctx.set_fill_style(&JsValue::from_str(&s));
            self.ctx
                .fill_rect(rect.pos.x, rect.pos.y, rect.size.w, rect.size.h);
        }
        if let Some(s) = stroke_style {
            self.ctx.set_stroke_style(&JsValue::from_str(&s));
            self.ctx
                .stroke_rect(rect.pos.x, rect.pos.y, rect.size.w, rect.size.h);
        }
    }

    fn line(&self, width: Percent, a: Pos, b: Pos, stroke_style: impl Into<Cow<'static, str>>) {
        let a = self.to_abs_pos(a);
        let b = self.to_abs_pos(b);

        self.ctx
            .set_stroke_style(&JsValue::from_str(&stroke_style.into()));
        self.set_line_width(width);

        self.ctx.begin_path();
        self.ctx.move_to(a.x, a.y);
        self.ctx.line_to(b.x, b.y);
        self.ctx.stroke();
    }
}

trait Drawable: 'static {
    fn draw(&self, ctx: &Renderer);
    fn on_mouse_event(&mut self, _ctx: &Renderer, _pos: Pos, _ty: MouseEventType) {}
}

#[derive(Debug, Clone, Copy, derive_more::Add, derive_more::AddAssign)]
struct AbsolutePos {
    x: f64,
    y: f64,
}
impl AbsolutePos {
    const ZERO: Self = AbsolutePos { x: 0.0, y: 0.0 };
}
#[derive(Debug, Clone, Copy)]
struct AbsoluteSize {
    w: f64,
    h: f64,
}
impl std::ops::Sub<AbsolutePos> for AbsoluteSize {
    type Output = Self;
    fn sub(self, rhs: AbsolutePos) -> Self::Output {
        Self { w: self.w - rhs.x, h: self.h - rhs.y }
    }
}
#[derive(Debug, Clone, Copy)]
struct AbsoluteRect {
    pos: AbsolutePos,
    size: AbsoluteSize,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    derive_more::Add,
    derive_more::AddAssign,
    derive_more::Sub,
    derive_more::SubAssign,
    derive_more::Mul,
    derive_more::MulAssign,
    derive_more::Div,
    derive_more::DivAssign,
)]
struct Percent(NotNan<f64>);
impl Percent {
    const ZERO: Self = Self(unsafe { NotNan::new_unchecked(0.0) });
    const HALF: Self = Self(unsafe { NotNan::new_unchecked(50.0) });
    const FULL: Self = Self(unsafe { NotNan::new_unchecked(100.0) });
    fn new(v: f64) -> Self {
        Self(NotNan::new(v).unwrap())
    }
    fn value(&self) -> f64 {
        self.0.into_inner()
    }
    fn from_absolute(value: f64, ref_: f64) -> Self {
        Self(NotNan::new(value / ref_ * 100.0).unwrap())
    }
    fn to_absolute(self, ref_: f64) -> f64 {
        self.0.into_inner() / 100.0 * ref_
    }
}

/// 左上が 0, 0 右下が 1, 1
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    derive_more::Add,
    derive_more::AddAssign,
    derive_more::Sub,
    derive_more::SubAssign,
)]
struct Pos {
    x: Percent,
    y: Percent,
}
impl Pos {
    const ZERO: Self = Pos { x: Percent::ZERO, y: Percent::ZERO };
    const CENTER: Self = Pos { x: Percent::HALF, y: Percent::HALF };
    fn new(x: f64, y: f64) -> Pos {
        Pos { x: Percent::new(x), y: Percent::new(y) }
    }
    fn replace_y(self, y: Percent) -> Pos {
        Pos { x: self.x, y }
    }
    fn rotate(self, sheta: f64) -> Pos {
        use std::f64::consts::PI;
        let rad = sheta / 180.0 * PI;
        let (sin, cos) = f64::sin_cos(rad);
        let Self { x, y } = self;
        let (x, y) = (x.value(), y.value());

        Pos {
            x: Percent::new(x * cos - y * sin),
            y: Percent::new(x * sin + y * cos),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, derive_more::Div)]
struct Size {
    w: Percent,
    h: Percent,
}
impl Size {
    const FULL: Self = Size { w: Percent::FULL, h: Percent::FULL };
    fn new(w: f64, h: f64) -> Self {
        Self { w: Percent::new(w), h: Percent::new(h) }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rect {
    pos: Pos,
    size: Size,
}
impl Rect {
    const FULL: Rect = Rect { pos: Pos::ZERO, size: Size::FULL };
    fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { pos: Pos::new(x, y), size: Size::new(w, h) }
    }
    #[rustfmt::skip]
    fn contains(&self, pos: Pos) -> bool {
        self.pos.x < pos.x && pos.x < (self.pos.x + self.size.w) &&
        self.pos.y < pos.y && pos.y < (self.pos.y + self.size.h)
    }
    fn center(&self) -> Pos {
        Pos {
            x: self.pos.x + Percent::new(self.size.w.value() / 2.0),
            y: self.pos.y + Percent::new(self.size.h.value() / 2.0),
        }
    }
    fn from_center(pos: Pos, width: Percent) -> Self {
        let width = width.value();
        Self {
            pos: pos - Pos::new(width / 2.0, width / 2.0),
            size: Size::new(width, width),
        }
    }
    fn map_in(&self, s: Self, p: Pos) -> Pos {
        s.pos
            + Pos::new(
                s.size.w.value() * p.x.value() / 100.0,
                s.size.h.value() * p.y.value() / 100.0,
            )
    }
    /// 横幅を縮めて 1:1 にする
    fn a16_9_to_a1_1(&self) -> Self {
        let shouldbe = self.size.w.value() / 16.0 * 9.0;
        let sub = self.size.w.value() - shouldbe;
        let off = Percent::new(sub);
        Self {
            // なんでか知らんけど /2.0 すると合う、、、なんで、、、、？
            pos: Pos { x: self.pos.x + off / 2.0, y: self.pos.y },
            size: Size { w: self.size.w - off, h: self.size.h },
        }
    }
}

#[test]
fn rect_map_in_test() {
    let base = Rect::FULL;
    let sub = Rect {
        pos: Pos::new(45.0, 45.0),
        size: Size::new(10.0, 10.0),
    };
    assert_eq!(base.map_in(sub, Pos::CENTER), Pos::CENTER);
}

trait Movable: Drawable {
    fn rect(&self) -> Rect;
    fn move_(&mut self, pos: Pos);
}

struct MovableEntry {
    component: Box<dyn Movable>,
    selected: Option<Dragging>,
}

struct Dragging {
    old_pos: Pos,
    holding_from: Pos,
}

impl MovableEntry {
    fn new(c: impl Movable) -> Self {
        Self { component: Box::new(c), selected: None }
    }
}

#[derive(Default)]
struct MovementController {
    /// component の onclick は呼ばれない
    /// 各 component は 0,0 に描画すること
    entries: Vec<MovableEntry>,
}
impl MovementController {
    fn push(&mut self, movable: impl Movable) {
        self.entries.push(MovableEntry::new(movable));
    }
}
impl Drawable for MovementController {
    fn on_mouse_event(&mut self, ctx: &Renderer, pos: Pos, ty: MouseEventType) {
        let overlap = self.entries.iter_mut().find(|x| {
            // let pos = ctx.to_abs_pos(pos);
            // let ctx = ctx.translate(x.base);
            // let pos = ctx.to_rel_pos(pos);
            x.component.rect().contains(pos)
        });

        match ty {
            MouseEventType::Down => {
                if let Some(entry) = overlap {
                    change_cursor_state(CursorState::Grabbing);

                    entry.selected = Some(Dragging {
                        old_pos: entry.component.rect().pos,
                        holding_from: pos,
                    });
                }
            }
            MouseEventType::Move => {
                change_cursor_state(if overlap.is_some() {
                    CursorState::Grab
                } else {
                    CursorState::Normal
                });

                if let Some(entry) = self.entries.iter_mut().find(|x| x.selected.is_some()) {
                    change_cursor_state(CursorState::Grabbing);

                    let dragging = entry.selected.as_ref().unwrap();
                    entry
                        .component
                        .move_(dragging.old_pos - dragging.holding_from + pos);
                }
            }
            MouseEventType::Up => {
                if let Some(entry) = self.entries.iter_mut().find(|x| x.selected.is_some()) {
                    change_cursor_state(CursorState::Grab);
                    entry.selected = None;
                }
            }
            MouseEventType::Click => {}
        }
    }

    fn draw(&self, ctx: &Renderer) {
        for entry in &self.entries {
            entry.component.draw(ctx);

            if entry.selected.is_some() {
                let _restore = ctx.dotted_line();
                ctx.set_line_width(Percent::new(0.14));
                ctx.rect(entry.component.rect(), None, Cow::from("black"));
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum TextAlign {
    TopLeft,
    Center,
    BottomLeft,
}

struct Text {
    pos: Pos,
    align: TextAlign,
    text: Cow<'static, str>,
    /// 画面の高さ基準。今回はアス比を固定しているので問題ない
    size: Percent,
}

impl Drawable for Text {
    fn draw(&self, ctx: &Renderer) {
        ctx.set_text_align(self.align);
        ctx.set_font_size(self.size);
        ctx.filled_text(&self.text, self.pos, Cow::from("black"));
    }
}

struct Button {
    rect: Rect,
    text: Cow<'static, str>,
}

impl Drawable for Button {
    fn draw(&self, ctx: &Renderer) {
        ctx.rect(self.rect, Cow::from("white"), Cow::from("black"));
        ctx.set_text_align(TextAlign::Center);
        ctx.set_font_to_fit(&self.text, self.rect.size.w - Percent::new(2.0));
        ctx.filled_text(&self.text, self.rect.center(), Cow::from("black"));
    }
}

#[derive(Clone, Copy)]
struct Port {
    pos: Pos,
}

trait CircuitComponent: Movable {
    fn ports(&self) -> Vec<Port>;
}

#[derive(Clone, Copy)]
struct Led {
    rect: Rect,
    port: Port,
}

impl Led {
    fn new() -> Self {
        let rect = Rect { pos: Pos::CENTER, size: Size::new(20.0, 20.0) };
        Self {
            rect,
            port: Port { pos: Rect::FULL.map_in(rect, Pos::new(3.0, 50.0)) },
        }
    }
}

impl Movable for Led {
    fn rect(&self) -> Rect {
        self.rect
    }

    fn move_(&mut self, pos: Pos) {
        self.rect.pos = pos;
        self.port.pos = Rect::FULL.map_in(self.rect, Pos::new(3.0, 50.0));
    }
}

impl CircuitComponent for Led {
    fn ports(&self) -> Vec<Port> {
        vec![self.port]
    }
}

impl Drawable for Led {
    fn draw(&self, ctx: &Renderer) {
        // self.movable.draw(ctx);
        tracing::info!(?self.rect);

        let ctx = ctx.subcanbas(self.rect);
        let w = Percent::new(1.0);
        let c = 50.0;

        let start = Pos::new(3.0, 50.0);
        let end = Pos::new(90.0, 50.0);

        // 横線
        ctx.line(w, start, end, "black");

        // GND
        for (i, &offx) in [10.0, 5.0, 3.0].iter().enumerate() {
            let i = i as f64;
            ctx.line(
                w,
                Pos::new(end.x.value() + i * 3.0, c - offx),
                Pos::new(end.x.value() + i * 3.0, c + offx),
                "black",
            );
        }

        let offx = 20.0 / 2.0;
        let offy = 40.0 / 2.0;
        let triangle = [
            Pos::new(c - offx, c + offy),
            Pos::new(c - offx, c - offy),
            Pos::new(c + offx, c),
        ];

        // 三角
        ctx.line(w, triangle[0], triangle[1], "black");
        ctx.line(w, triangle[1], triangle[2], "black");
        ctx.line(w, triangle[2], triangle[0], "black");

        // 三角の右の直線
        ctx.line(
            w,
            Pos::new(c + offx, c - offy),
            Pos::new(c + offx, c + offy),
            "black",
        );

        // 矢印
        let size = 25.0;
        let ctx = ctx.subcanbas(Rect::new(38.0, 8.0, size, size / 9.0 * 16.0));

        let draw_arrow = |start: Pos| {
            let off = Pos::new(20.0, -20.0);
            let w = Percent::new(4.0);
            ctx.line(w, start, start + off, "black");

            let len = 15.0;
            let d = Pos::new(-len, 0.0);
            ctx.line(w, start + off, start + off + d, "black");
            let d = Pos::new(0.0, len);
            ctx.line(w, start + off, start + off + d, "black");
        };

        let d = 14.0;
        draw_arrow(Pos::new(c + d, 50.0));
        draw_arrow(Pos::new(c - d, 50.0));
    }
}

struct Circuit {
    led_add_button: Button,
    movement: MovementController,
    components: Vec<CircuitComponentAdapter>,
}

impl Circuit {
    fn new() -> Self {
        Self {
            led_add_button: Button {
                rect: Rect::new(40.0, 90.0, 10.0, 10.0),
                text: Cow::from("LED"),
            },
            movement: MovementController::default(),
            components: vec![],
        }
    }
}

#[derive(Clone)]
struct CircuitComponentAdapter(Rc<RefCell<dyn CircuitComponent>>);
impl CircuitComponentAdapter {
    fn new(c: impl CircuitComponent) -> Self {
        Self(Rc::new(RefCell::new(c)))
    }
}

impl Drawable for CircuitComponentAdapter {
    fn draw(&self, ctx: &Renderer) {
        self.0.borrow().draw(ctx)
    }
}
impl Movable for CircuitComponentAdapter {
    fn rect(&self) -> Rect {
        self.0.borrow().rect()
    }

    fn move_(&mut self, pos: Pos) {
        self.0.borrow_mut().move_(pos)
    }
}
impl CircuitComponent for CircuitComponentAdapter {
    fn ports(&self) -> Vec<Port> {
        self.0.borrow().ports()
    }
}

impl Drawable for Circuit {
    fn on_mouse_event(&mut self, ctx: &Renderer, pos: Pos, ty: MouseEventType) {
        self.movement.on_mouse_event(ctx, pos, ty);
        for c in &mut self.components {
            c.on_mouse_event(ctx, pos, ty);
        }

        if let MouseEventType::Click = ty {
            if self.led_add_button.rect.contains(pos) {
                let led = CircuitComponentAdapter::new(Led::new());
                self.movement.push(led.clone());
                self.components.push(led);
            }
        }
    }

    fn draw(&self, ctx: &Renderer) {
        self.movement.draw(ctx);
        self.led_add_button.draw(ctx);

        for comp in &self.components {
            comp.draw(ctx);

            ctx.set_line_width(Percent::new(0.2));
            let ports = comp.ports();
            for p in ports {
                ctx.rect(
                    Rect::from_center(p.pos, Percent::new(2.0)).a16_9_to_a1_1(),
                    Cow::from("white"),
                    Cow::from("red"),
                );
            }
        }
    }
}
