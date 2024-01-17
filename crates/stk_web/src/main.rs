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
        let app = Rc::new(RefCell::new(App { canvas, main_scene: MainScene::new() }));

        let _resize_observer = ResizeObserver::new({
            let app = Rc::clone(&app);
            move |_entries| app.borrow_mut().on_resize()
        });
        _resize_observer.observe(&app.borrow().canvas);

        let _click_event = EventListener::new(&app.borrow().canvas, "click", {
            let app = Rc::clone(&app);
            move |event| app.borrow_mut().on_click(event)
        });

        let _mousemove_event = EventListener::new(&app.borrow().canvas, "mousemove", {
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
        let ctx = self.app.borrow().canvas.get_context("2d").unwrap().unwrap();
        let ctx: CanvasRenderingContext2d = ctx.dyn_into().unwrap();
        loop {
            self.app.borrow_mut().render(&ctx);
            RequestAnimationFrameFuture::new().await;
        }
    }
}

struct App {
    canvas: HtmlCanvasElement,
    main_scene: MainScene,
}

impl App {
    fn on_resize(&self) {
        let w = self.canvas.client_width() as u32;
        let h = self.canvas.client_height() as u32;
        self.canvas.set_width(w);
        self.canvas.set_height(h);
        tracing::info!("canvas resized to {w}x{h}");
    }

    fn mouse_event_to_pos(&self, m: &Event) -> AbsolutePos {
        let rect = self.canvas.get_bounding_client_rect();
        let event: &MouseEvent = m.dyn_ref().unwrap();
        let x = event.client_x() as f64 - rect.left();
        let y = event.client_y() as f64 - rect.top();
        AbsolutePos { x, y }
    }

    fn on_click(&self, event: &Event) {
        let pos = self.mouse_event_to_pos(event);
        tracing::info!("click: {}x{}", pos.x, pos.y);
    }

    fn on_mousemove(&self, event: &Event) {
        let pos = self.mouse_event_to_pos(event);
        tracing::info!("move: {}x{}", pos.x, pos.y);
    }

    fn render(&mut self, ctx: &CanvasRenderingContext2d) {
        self.main_scene.render(ctx);
    }
}

struct MainScene {
    i: usize,
}

impl MainScene {
    fn new() -> Self {
        Self { i: 0 }
    }

    fn on_cursormove(&self, x: f64, y: f64) {}

    fn render(&mut self, ctx: &CanvasRenderingContext2d) {
        let aspect_ratio = 16.0 / 9.0;
        let canvas = ctx.canvas().unwrap();
        let width = canvas.width() as f64;
        let height = canvas.height() as f64;

        ctx.set_fill_style(&JsValue::from_str("gray"));
        ctx.fill_rect(0.0, 0.0, width, height);
        ctx.set_text_baseline("top");
        ctx.set_text_align("left");

        let (size, offset) = {
            let a = AbsoluteSize { w: width, h: width / 16.0 * 9.0 };
            let b = AbsoluteSize { w: height / 9.0 * 16.0, h: height };
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
        LtoR(vecbox![Text {
            text: format!("{}", self.i).into(),
            size: Percent(5.0)
        },])
        .render(&ctx.translate(Pos { x: Percent(0.0), y: Percent(0.0) }))
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

    fn set_font_size(&self, size: Percent) {
        self.ctx
            .set_font(&format!("{}px sans-serif", size.to_absolute(self.size.h)));
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
    fn measure(&self, ctx: &Renderer<'_>) -> Size;
    fn render(&self, ctx: &Renderer<'_>);
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

#[derive(Debug, Clone, Copy)]
struct Percent(f64);
impl Percent {
    fn from_absolute(value: f64, ref_: f64) -> Self {
        Self(value / ref_ * 100.0)
    }
    fn to_absolute(self, ref_: f64) -> f64 {
        self.0 / 100.0 * ref_
    }
}

/// 左上が 0, 0 右下が 1, 1
#[derive(Debug, Clone, Copy)]
struct Pos {
    x: Percent,
    y: Percent,
}
impl Pos {
    const ZERO: Self = Pos { x: Percent(0.0), y: Percent(0.0) };
}
#[derive(Debug, Clone, Copy)]
struct Size {
    w: Percent,
    h: Percent,
}
#[derive(Debug, Clone, Copy)]
struct Rect {
    pos: Pos,
    size: Size,
}

struct LtoR(Vec<Box<dyn Drawable>>);

impl Drawable for LtoR {
    fn measure(&self, ctx: &Renderer<'_>) -> Size {
        let mut w = 0.0f64;
        let mut h = 0.0f64;

        for d in &self.0 {
            let d = d.measure(ctx);
            w += d.w.0;
            h = h.max(d.h.0);
        }

        Size { w: Percent(w), h: Percent(h) }
    }

    fn render(&self, ctx: &Renderer<'_>) {
        let mut x = 0.0;
        for d in &self.0 {
            d.render(&ctx.translate(Pos { x: Percent(x), y: Percent(0.0) }));
            x += d.measure(ctx).w.0;
        }
    }
}

struct Text {
    text: Cow<'static, str>,
    /// 画面の高さ基準。今回はアス比を固定しているので問題ない
    size: Percent,
}

impl Drawable for Text {
    fn measure(&self, ctx: &Renderer<'_>) -> Size {
        ctx.set_font_size(self.size);
        ctx.measure_text(&self.text)
    }

    fn render(&self, ctx: &Renderer<'_>) {
        ctx.set_font_size(self.size);
        ctx.filled_text(&self.text, Pos::ZERO, Cow::from("black"));
    }
}

struct Button {
    on: bool,
    size: Size,
}

impl Drawable for Button {
    fn measure(&self, _ctx: &Renderer<'_>) -> Size {
        self.size
    }

    fn render(&self, ctx: &Renderer<'_>) {
        ctx.rect(
            Rect { pos: Pos::ZERO, size: self.size },
            Cow::from(if self.on { "blue" } else { "red" }),
            Cow::from("black"),
        );
    }
}
