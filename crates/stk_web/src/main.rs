use stylist::yew::use_style;
use yew::prelude::*;

#[function_component]
fn App() -> Html {
    let s = use_style!(color: red;);
    html! {
        <h1 class={s}>{"Hello!"}</h1>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
