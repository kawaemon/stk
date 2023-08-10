mod bitmaskeq;
mod struct_map;

use proc_macro::TokenStream;

#[proc_macro]
pub fn struct_map(input: TokenStream) -> TokenStream {
    struct_map::struct_map(input)
}

#[proc_macro]
pub fn bitmaskeq(input: TokenStream) -> TokenStream {
    bitmaskeq::bitmaskeq(input)
}
