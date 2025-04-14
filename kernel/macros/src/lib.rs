use proc_macro2::Ident;
use quote::{quote, ToTokens};

struct Handler(usize);

const PUSH_ALL: &str = r"
push rax
push rcx
push rdx
push rbx
push rdi
push rsi
push rbp
push r8
push r9
push r10
push r11
push r12
push r13
push r14
push r15";

const POP_ALL: &str = r"
pop r15
pop r14
pop r13
pop r12
pop r11
pop r10
pop r9
pop r8
pop rbp
pop rsi
pop rdi
pop rbx
pop rdx
pop rcx
pop rax";

impl ToTokens for Handler {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let n = self.0;
        let ident = Ident::new(
            &format!("_interrupt_handler_{}", n), proc_macro2::Span::call_site());
        let source = if n < 32 {
            if [8, 10, 11, 12, 13, 14, 17, 21, 29, 30].contains(&n) {
                include_str!("err_handler.asm")
            } else {
                include_str!("exc_handler.asm")
            }
        } else if n < 0x40 {
            include_str!("int_handler.asm")
        } else {
            // TODO: apic acknowledge
            include_str!("int_high_handler.asm")
        }
            .replace("?push_all", PUSH_ALL)
            .replace("?pop_all", POP_ALL)
            .replace("?out_mid", if n >= 0x28 { "out 0xa0, al" } else { "" })
            .replace("?i", &ident.to_string());
        tokens.extend(quote! {
            #[naked]
            pub unsafe extern "C" fn #ident() {
                unsafe {
                    core::arch::naked_asm!(#source);
                }
            }
        });
    }
}

#[proc_macro]
pub fn interrupt_handlers(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let handlers = (0..256).map(Handler).collect::<Vec<_>>();

    quote! {
        #(#handlers)*
    }.into()
}

struct HandlerRef(usize);

impl ToTokens for HandlerRef {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let n = &self.0;
        let ident = Ident::new(
            &format!("_interrupt_handler_{}", n), proc_macro2::Span::call_site());
        tokens.extend(quote! {
            unsafe { core::mem::transmute(#ident as unsafe extern "C" fn() -> ()) }
        });
    }
}

#[proc_macro]
pub fn interrupt_handlers_arr(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let refs = (0..256).map(HandlerRef).collect::<Vec<_>>();

    quote! {
        [
            #(#refs),*
        ]
    }.into()
}
