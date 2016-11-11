#![recursion_limit = "100"]

extern crate inflections;
extern crate svd_parser as svd;
#[macro_use]
extern crate quote;
extern crate syn;

use std::io;
use std::io::Write;

use quote::Tokens;
use syn::*;

use inflections::Inflect;
use svd::{Access, Defaults, Peripheral, Register};

pub fn gen_peripheral(p: &Peripheral, d: &Defaults) -> Vec<Tokens> {
    if let Some(ref derived_from) = p.derived_from {
        let p_name = Ident::new(p.name.to_pascal_case());
        let derived_name = format!("::{}::{}",
                                   derived_from.to_snake_case(),
                                   derived_from.to_pascal_case());
        let derived_ty = Ident::new(derived_name);

        let type_alias = quote! {
            pub type #p_name = #derived_ty;
        };
        return vec![type_alias];
    }

    let mut items = vec![];
    let mut fields = vec![];
    let mut offset = 0;
    let mut i = 0;
    let registers = p.registers
        .as_ref()
        .expect(&format!("{:#?} has no `registers` field", p));
    for register in registers {
        let pad = if let Some(pad) = register.address_offset
            .checked_sub(offset) {
            pad
        } else {
            writeln!(io::stderr(),
                     "WARNING {} overlaps with another register at offset \
                      {}. Ignoring.",
                     register.name,
                     register.address_offset)
                .ok();
            continue;
        };

        if pad != 0 {
            let name = Ident::new(format!("_reserved{}", i));
            let pad = pad as usize;
            fields.push(quote! {
                #name : [u8; #pad]
            });
            i += 1;
        }

        let comment = &format!("0x{:02x} - {}",
                               register.address_offset,
                               respace(&register.description))[..];

        let field_ty = Ident::new(register.name.to_pascal_case());
        let field_name = Ident::new(register.name.to_snake_case());
        let field_access = register.access.unwrap_or_else(|| {
            let fields = register.fields
                .as_ref()
                .expect(&format!("{:#?} has no `fields` field", register));
            if fields.iter().all(|f| f.access == Some(Access::ReadOnly)) {
                Access::ReadOnly
            } else if fields.iter().all(|f| f.access == Some(Access::WriteOnly)) {
                Access::WriteOnly
            } else if fields.iter().any(|f| f.access == Some(Access::ReadWrite)) {
                Access::ReadWrite
            } else {
                panic!("unexpected case: {:#?}",
                       fields.iter().map(|f| f.access).collect::<Vec<_>>())
            }
        });
        let field_access_ty = match field_access {
            Access::ReadOnly => Ident::new("volatile::ReadOnly"),
            Access::WriteOnly => Ident::new("volatile::WriteOnly"),
            Access::ReadWrite => Ident::new("volatile::ReadWrite"),
            a => panic!("{:?} registers are not supported", a),
        };

        fields.push(quote! {
            #[doc = #comment]
            pub #field_name : #field_access_ty<#field_ty>
        });

        offset = register.address_offset +
                 register.size
            .or(d.size)
            .expect(&format!("{:#?} has no `size` field", register)) / 8;
    }

    let p_name = Ident::new(p.name.to_pascal_case());

    if let Some(description) = p.description.as_ref() {
        let comment = &respace(description)[..];
        items.push(quote! {
            #[doc = #comment]
        });
    }

    let struct_ = quote! {
        #[repr(C)]
        pub struct #p_name {
            #(#fields),*
        }
    };

    items.push(struct_);

    for register in registers {
        items.extend(gen_register(register, d));
        items.extend(gen_register_read_methods(register, d));
        items.extend(gen_register_write_methods(register, d));
    }

    items
}

pub fn gen_register(r: &Register, d: &Defaults) -> Vec<Tokens> {
    let mut items = vec![];

    let name = Ident::new(r.name.to_pascal_case());
    let bits_ty = r.size
        .or(d.size)
        .expect(&format!("{:#?} has no `size` field", r))
        .to_ty();

    items.push(quote! {
        #[derive(Debug, Clone, Copy)]
        #[repr(C)]
        pub struct #name {
            bits: #bits_ty
        }
    });

    items
}

pub fn gen_register_read_methods(r: &Register, d: &Defaults) -> Vec<Tokens> {
    let mut items = vec![];

    let name = Ident::new(r.name.to_pascal_case());
    let bits_ty = r.size
        .or(d.size)
        .expect(&format!("{:#?} has no `size` field", r))
        .to_ty();

    let mut impl_items = vec![];

    if r.fields.is_some() {
        for field in r.fields
            .as_ref()
            .expect(&format!("{:#?} has no `fields` field", r)) {
            if let Some(Access::WriteOnly) = field.access {
                continue;
            }

            let mut field_name = field.name.to_snake_case();
            if field_name.as_str() == "match" {
                field_name += "_";
            }
            let field_name = Ident::new(field_name);
            let offset = field.bit_range.offset as u8;

            let width = field.bit_range.width as u8;

            if let Some(description) = field.description.as_ref() {
                let bits = if width == 1 {
                    format!("Bit {}", offset)
                } else {
                    format!("Bits {}:{}", offset, offset + width - 1)
                };

                let comment = &format!("{} - {}", bits, respace(description))[..];
                impl_items.push(quote! {
                #[doc = #comment]
            });
            }

            let item = if width == 1 {
                quote! {
                pub fn #field_name(&self) -> bool {
                    const OFFSET: u8 = #offset;

                    self.bits & (1 << OFFSET) != 0
                }
            }
            } else {
                let width_ty = width.to_ty();
                let mask: u64 = (1 << width) - 1;
                let mask = Lit::Int(mask, IntTy::Unsuffixed);

                quote! {
                pub fn #field_name(&self) -> #width_ty {
                    const MASK: #bits_ty = #mask;
                    const OFFSET: u8 = #offset;

                    ((self.bits >> OFFSET) & MASK) as #width_ty
                }
            }
            };

            impl_items.push(item);
        }
    }

    items.push(quote! {
        impl #name {
            #(#impl_items)*
        }
    });

    items
}

pub fn gen_register_write_methods(r: &Register, d: &Defaults) -> Vec<Tokens> {
    let mut items = vec![];

    let name = Ident::new(format!("{}", r.name.to_pascal_case()));
    let bits_ty = r.size
        .or(d.size)
        .expect(&format!("{:#?} has no `size` field", r))
        .to_ty();

    let mut impl_items = vec![];

    if let Some(reset_value) = r.reset_value.or(d.reset_value) {
        impl_items.push(quote! {
            /// Reset value
            pub fn reset_value() -> Self {
                #name { bits: #reset_value }
            }
        });
    }

    if r.fields.is_some() {
        for field in r.fields
            .as_ref()
            .expect(&format!("{:#?} has no `fields` field", r)) {
            if let Some(Access::ReadOnly) = field.access {
                continue;
            }

            let name = Ident::new(format!("set_{}", field.name.to_snake_case()));
            let offset = field.bit_range.offset as u8;

            let width = field.bit_range.width as u8;

            if let Some(description) = field.description.as_ref() {
                let bits = if width == 1 {
                    format!("Bit {}", offset)
                } else {
                    format!("Bits {}:{}", offset, offset + width - 1)
                };

                let comment = &format!("{} - {}", bits, respace(description))[..];
                impl_items.push(quote! {
                #[doc = #comment]
            });
            }

            let item = if width == 1 {
                quote! {
                pub fn #name(&mut self, value: bool) -> &mut Self {
                    const OFFSET: u8 = #offset;

                    if value {
                        self.bits |= 1 << OFFSET;
                    } else {
                        self.bits &= !(1 << OFFSET);
                    }
                    self
                }
            }
            } else {
                let width_ty = width.to_ty();
                let mask = (1 << width) - 1;
                let mask = Lit::Int(mask, IntTy::Unsuffixed);

                quote! {
                pub fn #name(&mut self, value: #width_ty) -> &mut Self {
                    const OFFSET: u8 = #offset;
                    const MASK: #width_ty = #mask;

                    self.bits &= !((MASK as #bits_ty) << OFFSET);
                    self.bits |= ((value & MASK) as #bits_ty) << OFFSET;
                    self
                }
            }
            };

            impl_items.push(item);
        }
    }

    items.push(quote! {
        impl #name {
            #(#impl_items)*
        }
    });

    items
}

trait U32Ext {
    fn to_ty(&self) -> Ident;
}

impl U32Ext for u32 {
    fn to_ty(&self) -> Ident {
        match *self {
            1...8 => Ident::new("u8"),
            9...16 => Ident::new("u16"),
            17...32 => Ident::new("u32"),
            _ => panic!("{}.to_ty()", *self),
        }
    }
}

fn respace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
