mod app;

use std::{cell::RefCell, rc::Rc};

use app::{Pos, Rect, Size, System};
use js_sys::Function;
use once_cell::sync::OnceCell;
use stylist::yew::use_style;
use tracing_subscriber::{
    fmt::{
        format::{FmtSpan, Pretty},
        time::UtcTime,
    },
    layer::SubscriberExt,
    util::SubscriberInitExt,
};
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use yew::prelude::*;

trait AsFunction {
    fn as_function(&self) -> &Function;
}
impl AsFunction for &Closure<dyn Fn()> {
    fn as_function(&self) -> &Function {
        self.as_ref().dyn_ref().unwrap()
    }
}
struct RcEq<T>(Rc<T>);
impl<T> PartialEq<RcEq<T>> for RcEq<T> {
    fn eq(&self, other: &RcEq<T>) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

struct CanvasSystem {
    el: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
}
impl System for CanvasSystem {
    fn screen_size(&self) -> app::Size {
        app::Size {
            w: self.el.width() as usize,
            h: self.el.height() as usize,
        }
    }

    fn set_drawing_color(&self, c: impl Into<app::Color>) {
        let c = JsValue::from_str(&c.into().color_code());
        self.ctx.set_fill_style(&c);
        self.ctx.set_stroke_style(&c);
    }

    fn fill_rect(&self, r: impl Into<Rect>) {
        let app::Rect { size: Size { w, h }, pos: Pos { x, y } } = r.into();
        self.ctx.fill_rect(x as _, y as _, w as _, h as _);
    }
}

#[function_component]
fn App() -> Html {
    let r = use_node_ref();
    let app = use_mut_ref(app::App::new);

    use_effect_with_deps(
        |(r, RcEq(app))| {
            let el = r.get().unwrap().dyn_into::<HtmlCanvasElement>().unwrap();
            let ctx = el
                .get_context("2d")
                .unwrap()
                .unwrap()
                .dyn_into::<CanvasRenderingContext2d>()
                .unwrap();

            let request_handle = Rc::new(RefCell::new(0));
            let renderer: Rc<OnceCell<Closure<dyn Fn()>>> = Rc::new(OnceCell::new());

            let render = {
                let request_handle = Rc::clone(&request_handle);
                let renderer = Rc::clone(&renderer);
                let app = Rc::clone(app);
                let sys = CanvasSystem { ctx, el };
                move || {
                    sys.el.set_width(sys.el.client_width() as u32);
                    sys.el.set_height(sys.el.client_height() as u32);
                    app.borrow_mut().render(&sys);
                    *request_handle.borrow_mut() = web_sys::window()
                        .unwrap()
                        .request_animation_frame(renderer.get().unwrap().as_function())
                        .unwrap();
                }
            };

            renderer
                .get_or_init(|| Closure::new(render))
                .as_function()
                .call0(&JsValue::UNDEFINED)
                .unwrap();

            move || {
                web_sys::window()
                    .unwrap()
                    .cancel_animation_frame(*request_handle.borrow())
                    .unwrap();
            }
        },
        (r.clone(), RcEq(app)),
    );

    let container = use_style! {
        width: 100%;
        height: 100%;
    };
    let canvas = use_style! {
        // border: solid red 2px;
        width: 100%;
        height: 100%;
    };
    html! {
        <div class={container}>
            <canvas class={canvas} ref={r} />
        </div>
    }
}

fn main() {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_timer(UtcTime::rfc_3339())
        .with_writer(tracing_web::MakeConsoleWriter)
        .with_span_events(FmtSpan::ACTIVE);
    let perf_layer = tracing_web::performance_layer().with_details_from_fields(Pretty::default());

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(perf_layer)
        .init();

    yew::Renderer::<App>::new().render();
}
