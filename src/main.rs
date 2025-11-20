use anyhow::{Context, Result};
use quote::ToTokens;
use std::{env, fs, path::Path};
use syn::{
	Attribute, File, ImplItem, Item, ItemEnum, ItemFn, ItemImpl, ItemStruct, ItemTrait, TraitItem,
	Visibility,
};
use walkdir::WalkDir;
#[allow(dead_code)]
struct TestStruct {
	pub i: Box<TestStruct>,
	j: Box<TestStruct>,
}

fn main() -> Result<()> {
	let root = env::args().nth(1).unwrap_or_else(|| ".".to_string());
	let root = Path::new(&root);

	for entry in WalkDir::new(root)
		.into_iter()
		.filter_map(|e| e.ok())
		.filter(|e| e.path().extension().map(|e| e == "rs").unwrap_or(false))
	{
		let path = entry.path();
		let src = fs::read_to_string(path)
			.with_context(|| format!("Failed to read {}", path.display()))?;
		let file: File =
			syn::parse_file(&src).with_context(|| format!("Failed to parse {}", path.display()))?;

		// ファイル見出し
		println!(
			"// ************* {}",
			path.strip_prefix(root).unwrap_or(path).display()
		);

		for item in file.items {
			print_item(&item, 0);
		}
	}

	Ok(())
}

fn indent(level: usize) -> String {
	"    ".repeat(level)
}

// /// doc コメントだけを再生（syn v2 の API に対応）
fn print_doc_attrs(attrs: &[Attribute], indent_level: usize) {
	let ind = indent(indent_level);
	for attr in attrs {
		if !attr.path().is_ident("doc") {
			continue;
		}
		// #[doc = "..."] をパース
		if let syn::Meta::NameValue(nv) = &attr.meta {
			if let syn::Expr::Lit(expr_lit) = &nv.value {
				if let syn::Lit::Str(doc) = &expr_lit.lit {
					println!("{ind}/// {}", doc.value());
				}
			}
		}
	}
}

fn print_vis(vis: &Visibility) -> String {
	match vis {
		Visibility::Public(_) => "pub ".to_string(),
		Visibility::Restricted(_) => {
			// pub(crate) など
			format!("{} ", vis.to_token_stream())
		}
		Visibility::Inherited => "".to_string(),
	}
}

fn print_item(item: &Item, indent_level: usize) {
	match item {
		Item::Fn(f) => print_item_fn(f, indent_level),
		Item::Struct(s) => print_item_struct(s, indent_level),
		Item::Enum(e) => print_item_enum(e, indent_level),
		Item::Trait(t) => print_item_trait(t, indent_level),
		Item::Impl(i) => print_item_impl(i, indent_level),
		// 必要ならここに Type / Const / Mod などを追加
		_other => {
			//println!("{ind}// [skipped]", ind = indent(indent_level));
		}
	}
}

fn print_item_fn(f: &ItemFn, indent_level: usize) {
	let ind = indent(indent_level);
	print_doc_attrs(&f.attrs, indent_level);
	let sig = f.sig.to_token_stream().to_string();
	println!("{ind}{}{};", print_vis(&f.vis), sig);
}

fn print_item_struct(s: &ItemStruct, indent_level: usize) {
	let ind = indent(indent_level);
	print_doc_attrs(&s.attrs, indent_level);

	let vis = print_vis(&s.vis);
	let ident = &s.ident;
	let generics = &s.generics;
	let fields = &s.fields;
	let where_clause = s.generics.where_clause.as_ref();

	match fields {
		syn::Fields::Named(_) | syn::Fields::Unnamed(_) => {
			println!(
				"{ind}{}struct {}{} {}{}",
				vis,
				ident,
				generics.to_token_stream(),
				fields.to_token_stream(),
				where_clause
					.map(|w| w.to_token_stream().to_string())
					.unwrap_or_default()
			);
		}
		syn::Fields::Unit => {
			println!(
				"{ind}{}struct {}{};{}",
				vis,
				ident,
				generics.to_token_stream(),
				where_clause
					.map(|w| format!(" {}", w.to_token_stream()))
					.unwrap_or_default()
			);
		}
	}
}

fn print_item_enum(e: &ItemEnum, indent_level: usize) {
	let ind = indent(indent_level);
	print_doc_attrs(&e.attrs, indent_level);

	let vis = print_vis(&e.vis);
	let ident = &e.ident;
	let generics = &e.generics;
	let where_clause = e.generics.where_clause.as_ref();

	println!(
		"{ind}{}enum {}{}{} {{",
		vis,
		ident,
		generics.to_token_stream(),
		where_clause
			.map(|w| format!(" {}", w.to_token_stream()))
			.unwrap_or_default()
	);

	for v in &e.variants {
		print_doc_attrs(&v.attrs, indent_level + 1);
		let vind = indent(indent_level + 1);
		println!("{vind}{}", v.to_token_stream());
	}

	println!("{ind}}}");
}

fn print_item_trait(t: &ItemTrait, indent_level: usize) {
	let ind = indent(indent_level);
	print_doc_attrs(&t.attrs, indent_level);

	let vis = print_vis(&t.vis);
	let ident = &t.ident;
	let generics = &t.generics;
	let supertraits = &t.supertraits;
	let where_clause = t.generics.where_clause.as_ref();

	print!("{ind}{}trait {}{}", vis, ident, generics.to_token_stream());
	if !supertraits.is_empty() {
		print!(": {}", supertraits.to_token_stream());
	}
	if let Some(w) = where_clause {
		print!(" {}", w.to_token_stream());
	}
	println!(" {{");

	for item in &t.items {
		match item {
			TraitItem::Fn(m) => {
				print_doc_attrs(&m.attrs, indent_level + 1);
				let mind = indent(indent_level + 1);
				let sig = m.sig.to_token_stream().to_string();
				println!("{mind}fn {};", sig.trim_start_matches("fn "));
			}
			_ => {}
		}
	}

	println!("{ind}}}");
}

fn print_item_impl(i: &ItemImpl, indent_level: usize) {
	let ind = indent(indent_level);
	print_doc_attrs(&i.attrs, indent_level);

	let unsafety = i
		.unsafety
		.as_ref()
		.map(|u| u.to_token_stream().to_string() + " ");
	let generics = &i.generics;

	let header = if let Some((bang, path, for_token)) = &i.trait_ {
		// impl Trait for Type
		format!(
			"{}impl{} {} {} {} {}",
			unsafety.unwrap_or_default(),
			generics.to_token_stream(),
			bang.to_token_stream(),
			path.to_token_stream(),
			for_token.to_token_stream(),
			i.self_ty.to_token_stream()
		)
	} else {
		// impl Type
		format!(
			"{}impl{} {}",
			unsafety.unwrap_or_default(),
			generics.to_token_stream(),
			i.self_ty.to_token_stream()
		)
	};

	let where_clause = i.generics.where_clause.as_ref();
	if let Some(w) = where_clause {
		println!("{ind}{header} {} {{", w.to_token_stream());
	} else {
		println!("{ind}{header} {{");
	}

	for item in &i.items {
		match item {
			ImplItem::Fn(m) => {
				print_doc_attrs(&m.attrs, indent_level + 1);
				let mind = indent(indent_level + 1);
				let sig = m.sig.to_token_stream().to_string();
				println!("{mind}fn {};", sig.trim_start_matches("fn "));
			}
			_ => {}
		}
	}
	println!("{ind}}}");
}
