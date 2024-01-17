mod bitmaskeq;

use proc_macro::TokenStream;

#[proc_macro]
pub fn bitmaskeq(input: TokenStream) -> TokenStream {
    bitmaskeq::bitmaskeq(input)
}
