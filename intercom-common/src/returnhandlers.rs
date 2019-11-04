use crate::methodinfo::ComArg;
use crate::prelude::*;
use crate::tyhandlers::{self, Direction, ModelTypeSystem, TypeContext};
use crate::utils;
use proc_macro2::Span;
use syn::Type;

/// Defines return handler for handling various different return type schemes.
pub trait ReturnHandler: ::std::fmt::Debug
{
    /// Returns the current type system. Used internally by the trait.
    fn type_system(&self) -> ModelTypeSystem;

    /// The return type of the original Rust method.
    fn rust_ty(&self) -> Type;

    /// The return type span.
    fn return_type_span(&self) -> Span;

    /// The return type for COM implementation.
    fn com_ty(&self) -> Type
    {
        tyhandlers::get_ty_handler(&self.rust_ty(), TypeContext::new(self.type_system()))
            .com_ty(self.return_type_span(), Direction::Retval)
    }

    /// Gets the return statement for converting the COM result into Rust
    /// return.
    fn com_to_rust_return(&self, _result: &Ident) -> TokenStream
    {
        quote!()
    }

    /// Gets the return statement for converting the Rust result into COM
    /// outputs.
    fn rust_to_com_return(&self, _result: &Ident) -> TokenStream
    {
        quote!()
    }

    /// Gets the COM out arguments that result from the Rust return type.
    fn com_out_args(&self) -> Vec<ComArg>
    {
        vec![]
    }
}

/// Void return type.
#[derive(Debug)]
struct VoidHandler(Span);
impl ReturnHandler for VoidHandler
{
    fn rust_ty(&self) -> Type
    {
        utils::unit_ty(self.0)
    }

    // Void types do not depend on the type system.
    fn type_system(&self) -> ModelTypeSystem
    {
        ModelTypeSystem::Automation
    }

    /// The return type span.
    fn return_type_span(&self) -> Span
    {
        self.0
    }
}

/// Simple return type with the return value as the immediate value.
#[derive(Debug)]
struct ReturnOnlyHandler(Type, ModelTypeSystem, Span);
impl ReturnHandler for ReturnOnlyHandler
{
    fn type_system(&self) -> ModelTypeSystem
    {
        self.1
    }

    fn rust_ty(&self) -> Type
    {
        self.0.clone()
    }

    fn return_type_span(&self) -> Span
    {
        self.2
    }

    fn com_to_rust_return(&self, result: &Ident) -> TokenStream
    {
        tyhandlers::get_ty_handler(&self.rust_ty(), TypeContext::new(self.1)).com_to_rust(
            result,
            self.2,
            Direction::Retval,
        )
    }

    fn rust_to_com_return(&self, result: &Ident) -> TokenStream
    {
        tyhandlers::get_ty_handler(&self.rust_ty(), TypeContext::new(self.1)).rust_to_com(
            result,
            self.2,
            Direction::Retval,
        )
    }

    fn com_out_args(&self) -> Vec<ComArg>
    {
        vec![]
    }
}

/// Result type that supports error info for the `Err` value. Converted to
/// `[retval]` on success or `HRESULT` + `IErrorInfo` on error.
#[derive(Debug)]
struct ErrorResultHandler
{
    retval_ty: Type,
    return_ty: Type,
    span: Span,
    type_system: ModelTypeSystem,
}

impl ReturnHandler for ErrorResultHandler
{
    fn type_system(&self) -> ModelTypeSystem
    {
        self.type_system
    }
    fn rust_ty(&self) -> Type
    {
        self.return_ty.clone()
    }
    fn return_type_span(&self) -> Span
    {
        self.span
    }
    fn com_ty(&self) -> Type
    {
        let ts = self.type_system.as_typesystem_type(self.span);
        syn::parse2(quote_spanned!(self.span=>
            < intercom::raw::HRESULT as
                intercom::type_system::ExternOutput< #ts >>
                    ::ForeignType ))
        .unwrap()
    }

    fn com_to_rust_return(&self, result: &Ident) -> TokenStream
    {
        // Format the final Ok value.
        // If there is only one, it should be a raw value;
        // If there are multiple value turn them into a tuple.
        let ok_values = get_rust_ok_values(self.com_out_args());
        let ok_tokens = if ok_values.len() != 1 {
            quote!( ( #( #ok_values ),* ) )
        } else {
            quote!( #( #ok_values )* )
        };

        // Return statement checks for S_OK (should be is_success) HRESULT and
        // yields either Ok or Err Result based on that.
        quote!(
            // TODO: HRESULT::succeeded
            if #result == intercom::raw::S_OK || #result == intercom::raw::S_FALSE {
                Ok( #ok_tokens )
            } else {
                return Err( intercom::load_error(
                        self.as_ref(),
                        &__intercom_iid,
                        #result ) );
            }
        )
    }

    fn rust_to_com_return(&self, result: &Ident) -> TokenStream
    {
        // Get the OK idents. We'll use v0, v1, v2, ... depending on the amount
        // of patterns we need for possible tuples.
        let ok_idents = self
            .com_out_args()
            .iter()
            .enumerate()
            .map(|(idx, _)| Ident::new(&format!("v{}", idx + 1), Span::call_site()))
            .collect::<Vec<_>>();

        // Generate the pattern for the Ok(..).
        // Tuples get (v0, v1, v2, ..) pattern while everything else is
        // represented with just Ok( v0 ) as there's just one parameter.
        let ok_pattern = {
            // quote! takes ownership of tokens if we allow so let's give them
            // by reference here.
            let rok_idents = &ok_idents;
            match self.retval_ty {
                Type::Tuple(_) => quote!( ( #( #rok_idents ),* ) ),

                // Non-tuples should have only one ident. Concatenate the vector.
                _ => quote!( #( #rok_idents )* ),
            }
        };

        let (ok_writes, err_writes) = write_out_values(&ok_idents, self.com_out_args(), self.span);
        quote!(
            match #result {
                Ok( #ok_pattern ) => { #( #ok_writes );*; intercom::raw::S_OK },
                Err( e ) => {
                    #( #err_writes );*;
                    intercom::store_error( e ).hresult
                },
            }
        )
    }

    fn com_out_args(&self) -> Vec<ComArg>
    {
        get_out_args_for_result(&self.retval_ty, self.span, self.type_system)
    }
}

fn get_out_args_for_result(
    retval_ty: &Type,
    span: Span,
    type_system: ModelTypeSystem,
) -> Vec<ComArg>
{
    match *retval_ty {
        // Tuples map to multiple out args, no [retval].
        Type::Tuple(ref t) => t
            .elems
            .iter()
            .enumerate()
            .map(|(idx, ty)| {
                ComArg::new(
                    Ident::new(&format!("__out{}", idx + 1), span),
                    ty.clone(),
                    span,
                    Direction::Out,
                    type_system,
                )
            })
            .collect::<Vec<_>>(),
        _ => vec![ComArg::new(
            Ident::new("__out", span),
            retval_ty.clone(),
            span,
            Direction::Retval,
            type_system,
        )],
    }
}

fn write_out_values(
    idents: &[Ident],
    out_args: Vec<ComArg>,
    span: Span,
) -> (Vec<TokenStream>, Vec<TokenStream>)
{
    let mut ok_tokens = vec![];
    let mut err_tokens = vec![];
    for (ident, out_arg) in idents.iter().zip(out_args) {
        let arg_name = out_arg.name;
        let ok_value = out_arg.handler.rust_to_com(ident, span, Direction::Out);
        let err_value = out_arg.handler.default_value();

        ok_tokens.push(quote!( *#arg_name = #ok_value ));
        err_tokens.push(quote!( *#arg_name = #err_value ));
    }

    (ok_tokens, err_tokens)
}

/// Gets the result as Rust types for a success return value.
fn get_rust_ok_values(out_args: Vec<ComArg>) -> Vec<TokenStream>
{
    let mut tokens = vec![];
    for out_arg in out_args {
        let value = out_arg
            .handler
            .com_to_rust(&out_arg.name, out_arg.span, Direction::Retval);
        tokens.push(value);
    }
    tokens
}

/// Resolves the correct return handler to use.
pub fn get_return_handler(
    retval_ty: &Option<Type>,
    return_ty: &Option<Type>,
    span: Span,
    type_system: ModelTypeSystem,
) -> Result<Box<dyn ReturnHandler>, ()>
{
    Ok(match (retval_ty, return_ty) {
        (&None, &None) => Box::new(VoidHandler(span)),
        (&None, &Some(ref ty)) => Box::new(ReturnOnlyHandler(ty.clone(), type_system, span)),
        (&Some(ref rv), &Some(ref rt)) => Box::new(ErrorResultHandler {
            retval_ty: rv.clone(),
            return_ty: rt.clone(),
            span,
            type_system,
        }),

        // Unsupported return scheme. Note we are using Result::Err instead of
        // Option::None here because having no return handler is unsupported
        // error case.
        _ => return Err(()),
    })
}
