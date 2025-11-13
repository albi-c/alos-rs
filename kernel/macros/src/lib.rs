use proc_macro2::Ident;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, LitInt, Token};
use syn::parse::{Parse, ParseStream};

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
        let n: u16 = self.0.try_into().unwrap();
        let ident = Ident::new(
            &format!("_interrupt_handler_{}", n), proc_macro2::Span::call_site());
        tokens.extend(match n {
            0x00..0x20 if [8, 10, 11, 12, 13, 14, 17, 21, 29, 30].contains(&n) => quote! {
                #[unsafe(no_mangle)]
                extern "x86-interrupt" fn #ident(frame: crate::interrupts::InterruptStackFrame, error_code: u64) {
                    exc_handler(&frame, #n, error_code);
                }
            },
            0x00..0x20 => quote! {
                #[unsafe(no_mangle)]
                extern "x86-interrupt" fn #ident(frame: crate::interrupts::InterruptStackFrame) {
                    exc_handler(&frame, #n, 0);
                }
            },
            0x20..0x28 => quote! {
                #[unsafe(no_mangle)]
                extern "x86-interrupt" fn #ident(frame: crate::interrupts::InterruptStackFrame) {
                    irq_handler(&frame, #n);
                    unsafe { asm!("out 0x20, al", in("al") 0x20u8, options(nomem, nostack)); }
                }
            },
            0x28..0x30 => quote! {
                #[unsafe(no_mangle)]
                extern "x86-interrupt" fn #ident(frame: crate::interrupts::InterruptStackFrame) {
                    irq_handler(&frame, #n);
                    unsafe { asm!("out 0xa0, al\nout 0x20, al", in("al") 0x20u8, options(nomem, nostack)); }
                }
            },
            0x30..0xff => quote! {
                #[unsafe(no_mangle)]
                extern "x86-interrupt" fn #ident(frame: crate::interrupts::InterruptStackFrame) {
                    irq_handler(&frame, #n);
                }
            },
            0xff => quote! {
                #[unsafe(no_mangle)]
                extern "x86-interrupt" fn #ident(_frame: crate::interrupts::InterruptStackFrame) {}
            },
            _ => panic!("Invalid interrupt number: {}", n),
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
            #ident as u64
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

struct SlabsInput(Ident, usize, usize);

impl Parse for SlabsInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<Ident>()?;
        input.parse::<Token![:]>()?;
        let start = input.parse::<LitInt>()?.base10_parse::<usize>()?;
        input.parse::<Token![..]>()?;
        let end = input.parse::<LitInt>()?.base10_parse::<usize>()?;
        Ok(SlabsInput(ident, start, end))
    }
}

struct SlabsSlab(usize, Ident);

impl ToTokens for SlabsSlab {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let SlabsSlab(shift, ident) = self;
        let size: usize = 1 << shift;
        tokens.extend(quote! {
            #ident: SlabAllocator<#size>,
        })
    }
}

struct SlabsSlabInit(Ident);

impl ToTokens for SlabsSlabInit {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let SlabsSlabInit(ident) = self;
        tokens.extend(quote! {
            #ident: SlabAllocator::new(),
        })
    }
}

#[proc_macro]
pub fn slabs(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let SlabsInput(ident, start, end) = parse_macro_input!(input as SlabsInput);
    let slabs = (start..=end).map(|i| SlabsSlab(
        i, Ident::new(&format!("slab_{}", i), proc_macro2::Span::call_site()))).collect::<Vec<_>>();
    let slab_init = slabs.iter().map(|slab| SlabsSlabInit(slab.1.clone()));
    let slab_alloc_match = slabs.iter().map(|slab| {
        let SlabsSlab(shift, ident) = slab;
        if *shift == start {
            quote! {
                0usize..=#shift => self.#ident.allocate(),
            }
        } else {
            quote! {
                #shift => self.#ident.allocate(),
            }
        }
    });
    let slab_dealloc_match = slabs.iter().map(|slab| {
        let SlabsSlab(shift, ident) = slab;
        if *shift == start {
            quote! {
                0usize..=#shift => self.#ident.deallocate(memory.as_mut_array().unwrap()),
            }
        } else {
            quote! {
                #shift => self.#ident.deallocate(memory.as_mut_array().unwrap()),
            }
        }
    });
    quote! {
        #[derive(Default)]
        struct #ident {
            #(#slabs)*
        }

        impl #ident {
            const fn new() -> Self {
                Self {
                    #(#slab_init)*
                }
            }

            fn allocate(&mut self, size_shift: usize) -> &'static mut [u8] {
                match size_shift {
                    #(#slab_alloc_match)*
                    _ => panic!("Invalid slab shift: {}", size_shift)
                }
            }

            fn deallocate(&mut self, size_shift: usize, memory: &'static mut [u8]) {
                match size_shift {
                    #(#slab_dealloc_match)*
                    _ => panic!("Invalid slab shift: {}", size_shift)
                }
            }
        }
    }.into()
}
