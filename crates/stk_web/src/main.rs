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
    CanvasRenderingContext2d, Element, Event, HtmlCanvasElement, MouseEvent, ResizeObserverEntry,
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

async fn run() {
    let canvas = document().get_element_by_id("main").unwrap();
    let canvas: HtmlCanvasElement = canvas.dyn_into().unwrap();

    RenderLoop::new(canvas).run().await;
}

struct RenderLoop {
    app: Rc<RefCell<App>>,
    _resize_observer: ResizeObserver,
    _click_event: EventListener,
    _mousemove_event: EventListener,
}

impl RenderLoop {
    fn new(canvas: HtmlCanvasElement) -> Self {
        let ctx = canvas.get_context("2d").unwrap().unwrap();
        let ctx: CanvasRenderingContext2d = ctx.dyn_into().unwrap();

        let app = Rc::new(RefCell::new(App { ctx, main_scene: MainScene::new() }));

        let _resize_observer = ResizeObserver::new({
            let app = Rc::clone(&app);
            move |_entries| app.borrow_mut().on_resize()
        });
        _resize_observer.observe(&canvas);

        let _click_event = EventListener::new(&canvas, "click", {
            let app = Rc::clone(&app);
            move |event| app.borrow_mut().on_click(event)
        });

        let _mousemove_event = EventListener::new(&canvas, "mousemove", {
            let app = Rc::clone(&app);
            move |event| app.borrow_mut().on_mousemove(event)
        });

        Self {
            app,
            _resize_observer,
            _click_event,
            _mousemove_event,
        }
    }

    async fn run(&mut self) {
        loop {
            self.app.borrow_mut().render();
            RequestAnimationFrameFuture::new().await;
        }
    }
}

struct App {
    ctx: CanvasRenderingContext2d,
    main_scene: MainScene,
}

impl App {
    fn on_resize(&self) {
        let canvas = self.ctx.canvas().unwrap();
        let w = canvas.client_width() as u32;
        let h = canvas.client_height() as u32;
        canvas.set_width(w);
        canvas.set_height(h);
        tracing::info!("canvas resized to {w}x{h}");
    }

    fn mouse_event_to_pos(&self, m: &Event) -> AbsolutePos {
        let rect = self.ctx.canvas().unwrap().get_bounding_client_rect();
        let event: &MouseEvent = m.dyn_ref().unwrap();
        let x = event.client_x() as f64 - rect.left();
        let y = event.client_y() as f64 - rect.top();
        AbsolutePos { x, y }
    }

    fn on_click(&mut self, event: &Event) {
        let pos = self.mouse_event_to_pos(event);
        // tracing::info!("click: {}x{}", pos.x, pos.y);

        self.main_scene
            .on_click(Renderer::new(&self.ctx).to_rel_pos(pos));
    }

    fn on_mousemove(&self, event: &Event) {
        let pos = self.mouse_event_to_pos(event);
        // tracing::info!("move: {}x{}", pos.x, pos.y);
    }

    fn render(&mut self) {
        self.main_scene.render(&self.ctx);
    }
}

struct MainScene {
    i: usize,
    button: Button,
}

impl MainScene {
    fn new() -> Self {
        Self {
            i: 0,
            button: Button {
                rect: Rect::new(45.0, 45.0, 10.0, 10.0),
                on: false,
                text: "テストボタン".into(),
            },
        }
    }

    fn on_click(&mut self, pos: Pos) {
        self.button.on_click(pos);
    }

    fn render(&mut self, ctx: &CanvasRenderingContext2d) {
        let canvas = ctx.canvas().unwrap();
        let width = canvas.width() as f64;
        let height = canvas.height() as f64;

        ctx.set_fill_style(&JsValue::from_str("gray"));
        ctx.fill_rect(0.0, 0.0, width, height);

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
        let ctx = ctx.translate(ctx.to_rel_pos(offset));

        ctx.rect(
            Rect { pos: Pos::ZERO, size: ctx.to_rel_size(size) },
            Cow::from("white"),
            Cow::from("green"),
        );

        self.i += 1;

        // debug
        LtoR {
            base: Pos::ZERO,
            components: vecbox![Text {
                pos: Pos::ZERO,
                align: TextAlign::TopLeft,
                text: format!("{}", self.i).into(),
                size: Percent::new(5.0)
            }],
        }
        .draw(&ctx);
        self.button.draw(&ctx);
    }
}

macro_rules! vecbox {
    ($($el:expr),*$(,)?) => { vec![$(Box::new($el)),*] };
}
use vecbox;

struct Renderer<'a> {
    // ctx.translate だと subcanvas の subcanvas がむずそうなのでやめた
    offset: AbsolutePos,
    /// キャンバス全体のサイズ
    size: AbsoluteSize,
    /// このキャンバスのサイズ
    this_size: AbsoluteSize,
    ctx: &'a CanvasRenderingContext2d,
}

impl<'a> Renderer<'a> {
    fn new(ctx: &'a CanvasRenderingContext2d) -> Self {
        let canvas = ctx.canvas().unwrap();
        let size = AbsoluteSize {
            w: canvas.width() as f64,
            h: canvas.height() as f64,
        };
        Self {
            offset: AbsolutePos::ZERO,
            size,
            this_size: size,
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
            x: Percent::from_absolute(abs.x, self.size.w),
            y: Percent::from_absolute(abs.y, self.size.h),
        }
    }
    fn to_abs_pos(&self, rel: Pos) -> AbsolutePos {
        AbsolutePos {
            x: rel.x.to_absolute(self.size.w),
            y: rel.y.to_absolute(self.size.h),
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

    fn translate(&self, pos: Pos) -> Self {
        let pos = self.to_abs_pos(pos);
        Self {
            offset: self.offset + pos,
            this_size: self.this_size - pos,
            size: self.size,
            ctx: self.ctx,
        }
    }

    fn set_font_size_abs(&self, size: f64) {
        self.ctx.set_font(&format!("{size}px sans-serif"));
    }

    fn set_font_size(&self, size: Percent) {
        self.set_font_size_abs(size.to_absolute(self.size.h));
    }

    fn set_font_to_fit(&self, text: &str, width: Percent) {
        let width = width.to_absolute(self.size.w);

        self.set_font_size_abs(1.0);
        let size = self.ctx.measure_text(text).unwrap();
        self.set_font_size_abs(width / size.width());
    }

    fn set_text_align(&self, mode: TextAlign) {
        let (baseline, align) = match mode {
            TextAlign::TopLeft => ("top", "left"),
            TextAlign::Center => ("middle", "center"),
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

    fn filled_text(&self, text: &str, pos: Pos, fill_style: impl Into<Option<Cow<'static, str>>>) {
        let pos = self.to_abs_pos(pos);
        if let Some(s) = fill_style.into() {
            self.ctx.set_fill_style(&JsValue::from_str(&s));
        }
        self.ctx
            .fill_text(text, self.offset.x + pos.x, self.offset.y + pos.y)
            .unwrap();
    }

    fn rect(
        &self,
        rect: Rect,
        fill_style: impl Into<Option<Cow<'static, str>>>,
        stroke_style: impl Into<Option<Cow<'static, str>>>,
    ) {
        let mut rect = self.to_abs_rect(rect);
        rect.pos += self.offset;

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

    fn line(&self, width: Percent, a: Pos, b: Pos) {
        let a = self.to_abs_pos(a);
        let b = self.to_abs_pos(b);

        self.ctx.set_line_width(width.to_absolute(self.size.h));
        self.ctx.begin_path();
        self.ctx.move_to(a.x, a.y);
        self.ctx.line_to(b.x, b.y);
        self.ctx.stroke();
    }
}

trait Drawable: 'static {
    fn draw(&self, ctx: &Renderer<'_>);
    fn size(&self, ctx: &Renderer<'_>) -> Size;

    /// クリックハンドラ。実際に自分がクリックされたかどうか、呼び出し元は関知しない。
    fn on_click(&mut self, _pos: Pos) {}
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
)]
struct Percent(NotNan<f64>);
impl Percent {
    const ZERO: Self = Self(unsafe { NotNan::new_unchecked(0.0) });
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
#[derive(Debug, Clone, Copy)]
struct Pos {
    x: Percent,
    y: Percent,
}
impl Pos {
    const ZERO: Self = Pos { x: Percent::ZERO, y: Percent::ZERO };
    fn new(x: f64, y: f64) -> Pos {
        Pos { x: Percent::new(x), y: Percent::new(y) }
    }
}
#[derive(Debug, Clone, Copy)]
struct Size {
    w: Percent,
    h: Percent,
}
impl Size {
    fn new(w: f64, h: f64) -> Self {
        Self { w: Percent::new(w), h: Percent::new(h) }
    }
}
#[derive(Debug, Clone, Copy)]
struct Rect {
    pos: Pos,
    size: Size,
}
impl Rect {
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
            y: self.pos.x + Percent::new(self.size.h.value() / 2.0),
        }
    }
}

struct LtoR {
    base: Pos,
    components: Vec<Box<dyn Drawable>>,
}

impl Drawable for LtoR {
    fn size(&self, ctx: &Renderer<'_>) -> Size {
        let mut w = Percent::ZERO;
        let mut h = Percent::ZERO;

        for d in &self.components {
            let d = d.size(ctx);
            w += d.w;
            h = h.max(d.h);
        }

        Size { w, h }
    }

    fn draw(&self, ctx: &Renderer<'_>) {
        let mut pos = self.base;
        for d in &self.components {
            d.draw(&ctx.translate(pos));
            pos.x += d.size(ctx).w;
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum TextAlign {
    TopLeft,
    Center,
}

struct Text {
    pos: Pos,
    align: TextAlign,
    text: Cow<'static, str>,
    /// 画面の高さ基準。今回はアス比を固定しているので問題ない
    size: Percent,
}

impl Drawable for Text {
    fn size(&self, ctx: &Renderer<'_>) -> Size {
        ctx.measure_text(&self.text)
    }

    fn draw(&self, ctx: &Renderer<'_>) {
        ctx.set_text_align(self.align);
        ctx.set_font_size(self.size);
        ctx.filled_text(&self.text, self.pos, Cow::from("black"));
    }
}

struct Button {
    rect: Rect,
    on: bool,
    text: Cow<'static, str>,
}

impl Drawable for Button {
    fn on_click(&mut self, pos: Pos) {
        if self.rect.contains(pos) {
            self.on = !self.on;
        }
    }

    fn size(&self, _ctx: &Renderer<'_>) -> Size {
        self.rect.size
    }

    fn draw(&self, ctx: &Renderer<'_>) {
        ctx.rect(
            self.rect,
            None,
            Cow::from(if self.on { "red" } else { "black" }),
        );
        ctx.set_text_align(TextAlign::Center);
        ctx.set_font_to_fit(&self.text, self.rect.size.w - Percent::new(2.0));
        ctx.filled_text(&self.text, self.rect.center(), Cow::from("black"));
    }
}
