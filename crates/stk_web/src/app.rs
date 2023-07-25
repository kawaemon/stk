#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub w: usize,
    pub h: usize,
}
#[derive(Debug, Clone, Copy)]
pub struct Pos {
    pub x: usize,
    pub y: usize,
}
impl From<(usize, usize)> for Pos {
    fn from((x, y): (usize, usize)) -> Self {
        Pos { x, y }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub pos: Pos,
    pub size: Size,
}
impl Rect {
    pub fn two_pos(a: impl Into<Pos>, b: impl Into<Pos>) -> Self {
        let (a, b) = (a.into(), b.into());
        let w = b.x.checked_sub(a.x);
        let h = b.y.checked_sub(a.y);
        let (Some(w), Some(h)) = (w, h) else {
            panic!("not valid points: {a:?} and {b:?}")
        };
        Rect { pos: a, size: Size { w, h } }
    }
    pub fn as_two_pos(&self) -> (Pos, Pos) {
        let a = self.pos;
        let b = Pos { x: a.x + self.size.w, y: a.y + self.size.h };
        (a, b)
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}
impl Color {
    const GREEN: Color = Color::rgb(0, 255, 0);
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }
    pub fn color_code(&self) -> String {
        format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
    }
}
impl From<(u8, u8, u8)> for Color {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self::rgb(r, g, b)
    }
}

pub trait System {
    fn screen_size(&self) -> Size;
    fn set_drawing_color(&self, c: impl Into<Color>);
    fn fill_rect(&self, r: impl Into<Rect>);
}

pub struct App {}

impl App {
    pub fn new() -> Self {
        App {}
    }

    pub fn render(&mut self, system: &impl System) {
        system.set_drawing_color(Color::GREEN);
        system.fill_rect(Rect::two_pos((10, 10), (100, 100)));
    }
}
