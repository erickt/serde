use aster;

use syntax::ast::{
    Ident,
    MetaItem,
    Item,
    Expr,
    StructDef,
};
use syntax::ast;
use syntax::codemap::Span;
use syntax::ext::base::{Annotatable, ExtCtxt};
use syntax::ext::build::AstBuilder;
use syntax::ptr::P;

use field::struct_field_attrs;

pub fn expand_derive_serialize(
    cx: &mut ExtCtxt,
    span: Span,
    meta_item: &MetaItem,
    annotatable: &Annotatable,
    push: &mut FnMut(Annotatable)
) {
    let item = match *annotatable {
        Annotatable::Item(ref item) => item,
        _ => {
            cx.span_err(
                meta_item.span,
                "`derive` may only be applied to structs and enums");
            return;
        }
    };

    let builder = aster::AstBuilder::new().span(span);

    let generics = match item.node {
        ast::ItemStruct(_, ref generics) => generics,
        ast::ItemEnum(_, ref generics) => generics,
        _ => cx.bug("expected ItemStruct or ItemEnum in #[derive(Serialize)]")
    };

    let impl_generics = builder.from_generics(generics.clone())
        .add_ty_param_bound(
            builder.path().global().ids(&["serde", "ser", "Serialize"]).build()
        )
        .build();

    let ty = builder.ty().path()
        .segment(item.ident).with_generics(impl_generics.clone()).build()
        .build();

    let body = serialize_body(
        cx,
        &builder,
        &item,
        &impl_generics,
        builder.ty()
            .ref_()
            .lifetime("'__a")
            .build_ty(ty.clone()),
    );

    let where_clause = &impl_generics.where_clause;

    let impl_item = quote_item!(cx,
        #[automatically_derived]
        impl $impl_generics ::serde::ser::Serialize for $ty $where_clause {
            fn serialize<__S>(&self, serializer: &mut __S) -> ::std::result::Result<(), __S::Error>
                where __S: ::serde::ser::Serializer,
            {
                $body
            }
        }
    ).unwrap();

    push(Annotatable::Item(impl_item))
}

fn serialize_body(
    cx: &ExtCtxt,
    builder: &aster::AstBuilder,
    item: &Item,
    impl_generics: &ast::Generics,
    ty: P<ast::Ty>,
) -> P<ast::Expr> {
    match item.node {
        ast::ItemStruct(ref struct_def, _) => {
            serialize_item_struct(
                cx,
                builder,
                item,
                impl_generics,
                ty,
                struct_def,
            )
        }
        ast::ItemEnum(ref enum_def, _) => {
            serialize_item_enum(
                cx,
                builder,
                item.ident,
                impl_generics,
                ty,
                enum_def,
            )
        }
        _ => cx.bug("expected ItemStruct or ItemEnum in #[derive(Serialize)]")
    }
}

fn serialize_item_struct(
    cx: &ExtCtxt,
    builder: &aster::AstBuilder,
    item: &Item,
    impl_generics: &ast::Generics,
    ty: P<ast::Ty>,
    struct_def: &ast::StructDef,
) -> P<ast::Expr> {
    let mut named_fields = vec![];
    let mut unnamed_fields = 0;

    for field in struct_def.fields.iter() {
        match field.node.kind {
            ast::NamedField(name, _) => { named_fields.push(name); }
            ast::UnnamedField(_) => { unnamed_fields += 1; }
        }
    }

    match (named_fields.is_empty(), unnamed_fields == 0) {
        (true, true) => {
            serialize_unit_struct(
                cx,
                &builder,
                item.ident,
            )
        }
        (true, false) => {
            serialize_tuple_struct(
                cx,
                &builder,
                item.ident,
                impl_generics,
                ty,
                unnamed_fields,
            )
        }
        (false, true) => {
            serialize_struct(
                cx,
                &builder,
                item.ident,
                impl_generics,
                ty,
                struct_def,
                named_fields,
            )
        }
        (false, false) => {
            cx.bug("struct has named and unnamed fields")
        }
    }
}

fn serialize_unit_struct(
    cx: &ExtCtxt,
    builder: &aster::AstBuilder,
    type_ident: Ident
) -> P<ast::Expr> {
    let type_name = builder.expr().str(type_ident);

    quote_expr!(cx, serializer.visit_named_unit($type_name))
}

fn serialize_tuple_struct(
    cx: &ExtCtxt,
    builder: &aster::AstBuilder,
    type_ident: Ident,
    impl_generics: &ast::Generics,
    ty: P<ast::Ty>,
    fields: usize,
) -> P<ast::Expr> {
    let (visitor_struct, visitor_impl) = serialize_tuple_struct_visitor(
        cx,
        builder,
        ty.clone(),
        ty,
        fields,
        impl_generics,
    );

    let type_name = builder.expr().str(type_ident);

    quote_expr!(cx, {
        $visitor_struct
        $visitor_impl
        serializer.visit_named_seq($type_name, Visitor {
            value: self,
            state: 0,
            _structure_ty: ::std::marker::PhantomData,
        })
    })
}

fn serialize_struct(
    cx: &ExtCtxt,
    builder: &aster::AstBuilder,
    type_ident: Ident,
    impl_generics: &ast::Generics,
    ty: P<ast::Ty>,
    struct_def: &StructDef,
    fields: Vec<Ident>,
) -> P<ast::Expr> {
    let (visitor_struct, visitor_impl) = serialize_struct_visitor(
        cx,
        builder,
        ty.clone(),
        ty,
        struct_def,
        impl_generics,
        fields.iter().map(|field| quote_expr!(cx, &self.value.$field)),
    );

    let type_name = builder.expr().str(type_ident);

    quote_expr!(cx, {
        $visitor_struct
        $visitor_impl
        serializer.visit_named_map($type_name, Visitor {
            value: self,
            state: 0,
            _structure_ty: ::std::marker::PhantomData,
        })
    })
}

fn serialize_item_enum(
    cx: &ExtCtxt,
    builder: &aster::AstBuilder,
    type_ident: Ident,
    impl_generics: &ast::Generics,
    ty: P<ast::Ty>,
    enum_def: &ast::EnumDef,
) -> P<ast::Expr> {
    let arms: Vec<ast::Arm> = enum_def.variants.iter()
        .enumerate()
        .map(|(variant_index, variant)| {
            serialize_variant(
                cx,
                builder,
                type_ident,
                impl_generics,
                ty.clone(),
                variant,
                variant_index,
            )
        })
        .collect();

    quote_expr!(cx,
        match *self {
            $arms
        }
    )
}

fn serialize_variant(
    cx: &ExtCtxt,
    builder: &aster::AstBuilder,
    type_ident: Ident,
    generics: &ast::Generics,
    ty: P<ast::Ty>,
    variant: &ast::Variant,
    variant_index: usize,
) -> ast::Arm {
    let type_name = builder.expr().str(type_ident);
    let variant_ident = variant.node.name;
    let variant_name = builder.expr().str(variant_ident);

    match variant.node.kind {
        ast::TupleVariantKind(ref args) if args.is_empty() => {
            let pat = builder.pat().enum_()
                .id(type_ident).id(variant_ident).build()
                .build();

            quote_arm!(cx,
                $pat => {
                    ::serde::ser::Serializer::visit_enum_unit(
                        serializer,
                        $type_name,
                        $variant_index,
                        $variant_name,
                    )
                }
            )
        }
        ast::TupleVariantKind(ref args) => {
            let fields: Vec<ast::Ident> = (0 .. args.len())
                .map(|i| builder.id(format!("__field{}", i)))
                .collect();

            let pat = builder.pat().enum_()
                .id(type_ident).id(variant_ident).build()
                .with_pats(fields.iter().map(|field| builder.pat().ref_id(field)))
                .build();

            let expr = serialize_tuple_variant(
                cx,
                builder,
                type_name,
                variant_index,
                variant_name,
                generics,
                ty,
                args,
                fields,
            );

            quote_arm!(cx, $pat => { $expr })
        }
        ast::StructVariantKind(ref struct_def) => {
            let fields: Vec<_> = (0 .. struct_def.fields.len())
                .map(|i| builder.id(format!("__field{}", i)))
                .collect();

            let pat = builder.pat().struct_()
                .id(type_ident).id(variant_ident).build()
                .with_pats(
                    fields.iter()
                        .zip(struct_def.fields.iter())
                        .map(|(id, field)| {
                            let name = match field.node.kind {
                                ast::NamedField(name, _) => name,
                                ast::UnnamedField(_) => {
                                    cx.bug("struct variant has unnamed fields")
                                }
                            };

                            (name, builder.pat().ref_id(id))
                        })
                )
                .build();

            let expr = serialize_struct_variant(
                cx,
                builder,
                type_name,
                variant_index,
                variant_name,
                generics,
                ty,
                struct_def,
                fields,
            );

            quote_arm!(cx, $pat => { $expr })
        }
    }
}

fn serialize_tuple_variant(
    cx: &ExtCtxt,
    builder: &aster::AstBuilder,
    type_name: P<ast::Expr>,
    variant_index: usize,
    variant_name: P<ast::Expr>,
    generics: &ast::Generics,
    structure_ty: P<ast::Ty>,
    args: &[ast::VariantArg],
    fields: Vec<Ident>,
) -> P<ast::Expr> {
    let variant_ty = builder.ty().tuple()
        .with_tys(
            args.iter().map(|arg| {
                builder.ty()
                    .ref_()
                    .lifetime("'__a")
                    .build_ty(arg.ty.clone())
            })
        )
        .build();

    let (visitor_struct, visitor_impl) = serialize_tuple_struct_visitor(
        cx,
        builder,
        structure_ty,
        variant_ty,
        args.len(),
        generics,
    );

    let value_expr = builder.expr().tuple()
        .with_exprs(
            fields.iter().map(|field| {
                builder.expr()
                    .addr_of()
                    .id(field)
            })
        )
        .build();

    quote_expr!(cx, {
        $visitor_struct
        $visitor_impl
        serializer.visit_enum_seq($type_name, $variant_index, $variant_name, Visitor {
            value: $value_expr,
            state: 0,
            _structure_ty: ::std::marker::PhantomData,
        })
    })
}

fn serialize_struct_variant(
    cx: &ExtCtxt,
    builder: &aster::AstBuilder,
    type_name: P<ast::Expr>,
    variant_index: usize,
    variant_name: P<ast::Expr>,
    generics: &ast::Generics,
    structure_ty: P<ast::Ty>,
    struct_def: &ast::StructDef,
    fields: Vec<Ident>,
) -> P<ast::Expr> {
    let value_ty = builder.ty().tuple()
        .with_tys(
            struct_def.fields.iter().map(|field| {
                builder.ty()
                    .ref_()
                    .lifetime("'__a")
                    .build_ty(field.node.ty.clone())
            })
        )
        .build();

    let value_expr = builder.expr().tuple()
        .with_exprs(
            fields.iter().map(|field| {
                builder.expr()
                    .addr_of()
                    .id(field)
            })
        )
        .build();

    let (visitor_struct, visitor_impl) = serialize_struct_visitor(
        cx,
        builder,
        structure_ty,
        value_ty,
        struct_def,
        generics,
        (0 .. fields.len()).map(|i| {
            builder.expr()
                .tup_field(i)
                .field("value").self_()
        })
    );

    quote_expr!(cx, {
        $visitor_struct
        $visitor_impl
        serializer.visit_enum_map($type_name, $variant_index, $variant_name, Visitor {
            value: $value_expr,
            state: 0,
            _structure_ty: ::std::marker::PhantomData,
        })
    })
}

fn serialize_tuple_struct_visitor(
    cx: &ExtCtxt,
    builder: &aster::AstBuilder,
    structure_ty: P<ast::Ty>,
    variant_ty: P<ast::Ty>,
    fields: usize,
    generics: &ast::Generics
) -> (P<ast::Item>, P<ast::Item>) {
    let arms: Vec<ast::Arm> = (0 .. fields)
        .map(|i| {
            let expr = builder.expr()
                .tup_field(i)
                .field("value").self_();

            quote_arm!(cx,
                $i => {
                    self.state += 1;
                    let v = try!(serializer.visit_tuple_elt(&$expr));
                    Ok(Some(v))
                }
            )
        })
        .collect();

    let visitor_impl_generics = builder.from_generics(generics.clone())
        .add_lifetime_bound("'__a")
        .lifetime_name("'__a")
        .build();

    let where_clause = &visitor_impl_generics.where_clause;

    let visitor_generics = builder.from_generics(visitor_impl_generics.clone())
        .strip_bounds()
        .build();

    // Variants don't necessarily reference all generic lifetimes and type parameters,
    // so to avoid a compilation failure, we'll just add a phantom type to capture these
    // unused values.
    let structure_ty = builder.ty()
        .phantom_data()
        .build(structure_ty);

    (
        quote_item!(cx,
            struct Visitor $visitor_impl_generics $where_clause {
                state: usize,
                value: $variant_ty,
                _structure_ty: $structure_ty,
            }
        ).unwrap(),

        quote_item!(cx,
            impl $visitor_impl_generics ::serde::ser::SeqVisitor
            for Visitor $visitor_generics
            $where_clause {
                #[inline]
                fn visit<S>(&mut self, serializer: &mut S) -> ::std::result::Result<Option<()>, S::Error>
                    where S: ::serde::ser::Serializer
                {
                    match self.state {
                        $arms
                        _ => Ok(None)
                    }
                }

                #[inline]
                fn len(&self) -> Option<usize> {
                    Some($fields)
                }
            }
        ).unwrap(),
    )
}

fn serialize_struct_visitor<I>(
    cx: &ExtCtxt,
    builder: &aster::AstBuilder,
    structure_ty: P<ast::Ty>,
    variant_ty: P<ast::Ty>,
    struct_def: &StructDef,
    generics: &ast::Generics,
    value_exprs: I,
) -> (P<ast::Item>, P<ast::Item>)
    where I: Iterator<Item=P<ast::Expr>>,
{
    let len = struct_def.fields.len();

    let field_attrs = struct_field_attrs(cx, builder, struct_def);

    let arms: Vec<ast::Arm> = field_attrs.into_iter()
        .zip(value_exprs)
        .enumerate()
        .map(|(i, (field, value_expr))| {
            let key_expr = field.serializer_key_expr(cx);
            quote_arm!(cx,
                $i => {
                    self.state += 1;
                    Ok(
                        Some(
                            try!(
                                serializer.visit_named_map_elt(
                                    $key_expr,
                                    $value_expr,
                                )
                            )
                        )
                    )
                }
            )
        })
        .collect();

    let visitor_impl_generics = builder.from_generics(generics.clone())
        .add_lifetime_bound("'__a")
        .lifetime_name("'__a")
        .build();

    let where_clause = &visitor_impl_generics.where_clause;

    let visitor_generics = builder.from_generics(visitor_impl_generics.clone())
        .strip_bounds()
        .build();

    // Variants don't necessarily reference all generic lifetimes and type parameters,
    // so to avoid a compilation failure, we'll just add a phantom type to capture these
    // unused values.
    let structure_ty = builder.ty()
        .phantom_data()
        .build(structure_ty);

    (
        quote_item!(cx,
            struct Visitor $visitor_impl_generics $where_clause {
                state: usize,
                value: $variant_ty,
                _structure_ty: $structure_ty,
            }
        ).unwrap(),

        quote_item!(cx,
            impl $visitor_impl_generics
            ::serde::ser::MapVisitor
            for Visitor $visitor_generics
            $where_clause {
                #[inline]
                fn visit<S>(&mut self, serializer: &mut S) -> ::std::result::Result<Option<()>, S::Error>
                    where S: ::serde::ser::Serializer,
                {
                    match self.state {
                        $arms
                        _ => Ok(None)
                    }
                }

                #[inline]
                fn len(&self) -> Option<usize> {
                    Some($len)
                }
            }
        ).unwrap(),
    )
}
