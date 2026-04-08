use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

/// Derive macro that generates a `{TypeName}State` struct and a `reconstitute` associated function.
///
/// Apply `#[derive(Reconstitute)]` to a named-field struct to generate:
///
/// 1. A `{TypeName}State` struct with all the same fields, all `pub`
/// 2. A `pub fn reconstitute(state: {TypeName}State) -> Self` associated function
///    that maps every field from the state struct into the type
///
/// Fields annotated with `#[reconstitute_ignore]` are excluded from the generated
/// `State` struct and are populated via `Default::default()` in `reconstitute`.
///
/// # Example
///
/// ```ignore
/// #[derive(Reconstitute)]
/// pub struct Application {
///     id: ApplicationId,
///     name: String,
///     #[reconstitute_ignore]
///     pending_events: Vec<Event>,
/// }
/// ```
///
/// Generates:
///
/// ```ignore
/// pub struct ApplicationState {
///     pub id: ApplicationId,
///     pub name: String,
/// }
///
/// impl Application {
///     pub fn reconstitute(state: ApplicationState) -> Self {
///         Self {
///             id: state.id,
///             name: state.name,
///             pending_events: Default::default(),
///         }
///     }
/// }
/// ```
///
/// Only named-field structs are supported. Enums, tuple structs, unit structs, and
/// unions will produce a clear compile error.
#[proc_macro_derive(Reconstitute, attributes(reconstitute_ignore))]
pub fn derive_reconstitute(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let struct_name = &input.ident;
    let state_name = syn::Ident::new(&format!("{}State", struct_name), struct_name.span());
    let vis = &input.vis;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            Fields::Unnamed(_) => {
                return syn::Error::new_spanned(
                    struct_name,
                    "Reconstitute only supports structs with named fields, not tuple structs",
                )
                .to_compile_error()
                .into();
            }
            Fields::Unit => {
                return syn::Error::new_spanned(
                    struct_name,
                    "Reconstitute only supports structs with named fields, not unit structs",
                )
                .to_compile_error()
                .into();
            }
        },
        Data::Enum(_) => {
            return syn::Error::new_spanned(
                struct_name,
                "Reconstitute only supports structs, not enums",
            )
            .to_compile_error()
            .into();
        }
        Data::Union(_) => {
            return syn::Error::new_spanned(
                struct_name,
                "Reconstitute only supports structs, not unions",
            )
            .to_compile_error()
            .into();
        }
    };

    let state_fields = fields
        .iter()
        .filter(|f| !f.attrs.iter().any(|attr| attr.path().is_ident("reconstitute_ignore")))
        .map(|f| {
            let name = &f.ident;
            let ty = &f.ty;
            quote! { pub #name: #ty }
        });

    let field_mappings = fields.iter().map(|f| {
        let name = &f.ident;
        if f.attrs.iter().any(|attr| attr.path().is_ident("reconstitute_ignore")) {
            quote! { #name: Default::default() }
        } else {
            quote! { #name: state.#name }
        }
    });

    let expanded = quote! {
        #vis struct #state_name {
            #(#state_fields,)*
        }

        impl #struct_name {
            pub fn reconstitute(state: #state_name) -> Self {
                Self {
                    #(#field_mappings,)*
                }
            }
        }
    };

    expanded.into()
}
