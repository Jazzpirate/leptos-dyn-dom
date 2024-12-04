#![cfg_attr(feature="ssr",allow(unused_variables))]
#![cfg_attr(feature="ssr",allow(unused_mut))]
#![cfg_attr(feature="ssr",allow(unused_imports))]

/*! Allows for "hydrating" an existent DOM with reactive leptos components,
 * without the entire DOM having to be generated by leptos components.
 * 
 * ## Why would you want that?
 * 1. **CSR:** It allows for building scripts that others can just embed in their arbitrary HTML documents, that adds `<insert your favourite fancy feature here>`. For an example, see the `examples/csr` directory: the `index.html` has a node `<script src='csr_example.js'></script>`, which "hydrates" selected nodes (with the `data-replace-with-leptos`-attribute) with leptos components that add a hover-popup (using [thaw](https://docs.rs/thaw)). You too can now do the same for any HTML document you want by just adding the script tag.
 * 2. **SSR:** Occasionally, you might want to dynamically insert some HTML string into the DOM, for example one that gets generated from some data and returned by a server function. This HTML might contain certain nodes that we want to attach reactive functionality to.
 * 
 * ## CSR Example
 * Say we want to replace all elements with the attribute `data-replace-with-leptos` with a leptos component `MyReplacementComponent`, that simply wraps the original children in a `div` with a solid red border. This component would roughly look like this:
 * ```
 * #[component]
 * fn MyReplacementComponent(orig:OriginalNode) -> impl IntoView {
 *    view! {
 *       <div style="border: 1px solid red;">
 *         <DomChildren orig />
 *      </div>
 *   }
 * }
 * ```
 * This component takes an `orig:`[`OriginalChildren`] that represents the "children the original node used to have". They get reinserted where we use the [`DomChildren`] component - i.e. wrapped in a `div` with a red border.
 * 
 * So, where do we get `orig` from? 
 * - If we already have an `e:&`[`Element`], we can simply call [`OriginalChildren::new`]`(e)`. That will immediately remove *all* children of `e` and move them into the returned [`OriginalChildren`]. Trouble then is, that the component likely doesn't know where in leptos' reactive graph it should be inserted regarding reactivity (i.e. inheriting context and all that).
 * - More likely, we don't have an [`Element`] yet. Moreover, we probably want to iterate over the entire body *once* to find all nodes we want to make reactive, and we also need to set up a global reactive system for all our inserted components.
 * 
 * To do that, we call [`hydrate_body`] (requires the `csr` feature flag) with a function that takes the [`OriginalChildren`] of the body and returns some leptos view; e.g.:
 * 
 * ```
 *  #[component]
 *  fn MainBody(orig:OriginalNode) -> impl IntoView {
 *     // set up some signals, provide context etc.
 *     view!{
 *       <DomChildren orig/>
 *     }
 *  }
 *  #[wasm_bindgen(start)]
 *   pub fn run() {
 *       console_error_panic_hook::set_once();
 *       hydrate_body(|orig| view!(<MainBody orig/>).into_any())
 *   }
 * ```
 * 
 * This sets up the reactive system, but does not yet replace any elements further down in the DOM. To do that, we provide a function that takes an `&`[`Element`] and optionally returns an [`AnyView`]`<`[`Dom`]`>`, if the element should be changed. This function is then passed to [`DomChildrenCont`], which will iterate over all children of the replaced element and replace them with the provided function.
 * 
 * Let's modify our `MainBody` to replace all elements with the attribute `data-replace-with-leptos` with a `MyReplacementComponent`:
 * 
 * ```
 *  fn replace(e:&Element) -> Option<AnyView<Dom>> {
 *    e.get_attribute("data-replace-with-leptos").map(|_| {
 *      let orig = e.clone().into();
 *      view!(<MyReplacementComponent orig/>).into_any()
 *    })
 *  }
 * 
 *  #[component]
 *  fn MainBody(orig:OriginalNode) -> impl IntoView {
 *     // set up some signals, provide context etc.
 *     view!{
 *       <DomChildrenCont orig cont=replace/>
 *     }
 *  }
 * 
 * #[component]
 * fn MyReplacementComponent(orig:OriginalNode) -> impl IntoView {
 *    view! {
 *       <div style="border: 1px solid red;">
 *         <DomChildrenCont orig cont=replace/>
 *      </div>
 *   }
 * }
 * ```
 * 
 * ...now, `replace` will get called on every element of the DOM, including those that were "moved around" in earlier `MyReplacementComponent`s, respecting the reactive graph properly hierarchically.
 * 
 * ### SSR Example
 * 
 * In general, for SSR we can simply use the normal leptos components to generate the entire DOM. We control the server, hence we control the DOM anyway.
 * 
 * However, it might occasionally be the case that we want to dynamically *extend* the DOM at some point by retrieving HTML from elsewhere, and then want to do a similar "hydration" iteration over the freshly inserted nodes. This is what [`DomStringCont`] is for, and it does not require the `csr` feature:
 * 
 * ```
 *  #[component]
 *  fn MyComponentThatGetsAStringFromSomewhere() -> impl IntoView {
 *   // get some HTML string from somewhere
 *   // e.g. some API call
 *   let html = "<div data-replace-with-leptos>...</div>".to_string();
 *   view! {
 *     <DomStringCont html cont=replace/>
 *   }
 * }
 * ```
 * 
 * See the `examples/ssr` directory for a full example.
*/

mod node;
mod dom;

pub use node::{OriginalNode,AnyTag};

#[cfg(any(feature="csr",feature="hydrate"))]
pub use dom::hydrate_node;

use leptos::{web_sys::Element, html::Span, math::Mrow, prelude::*};
use send_wrapper::SendWrapper;

/// A component that calls `f` on all children of `orig`
/// to potentially "hydrate" them further, and reinserts the original
/// element into the DOM.
#[component]
pub fn DomCont<
    V:IntoView+'static,
    R:FnOnce() -> V,
    F:Fn(&Element) -> Option<R>+'static+Send
>(orig:OriginalNode,#[prop(optional)] skip_head:bool,cont:F,#[prop(optional)] on_load:Option<RwSignal<bool>>) -> impl IntoView {
    #[cfg(any(feature="csr",feature="hydrate"))]
    let mut inner = orig.inner.clone();
    orig.as_view(move |e| {
        #[cfg(any(feature="csr",feature="hydrate"))]
        {
            OriginalNode::do_self(&mut inner,e);
            let node:leptos::web_sys::Node = (*inner).clone().into();
            if skip_head {
                crate::cleanup(node.clone());
                dom::hydrate_children(node, &cont);
            } else {
                dom::hydrate_node(node, &cont);
            }
            if let Some(on_load) = on_load { on_load.set(true); }
        }
    })
}


/// A component that inserts the  children of some [`OriginalNode`] 
/// and renders them into the DOM.
#[component]
pub fn DomChildren(orig:OriginalNode,#[prop(optional)] on_load:Option<RwSignal<bool>>) -> impl IntoView {
    #[cfg(any(feature="csr",feature="hydrate"))]
    let inner = orig.inner.clone();
    orig.as_view(move |e| {
        #[cfg(any(feature="csr",feature="hydrate"))]
        {
            OriginalNode::do_children(&inner, e,cleanup);
            if let Some(on_load) = on_load { on_load.set(true); }
        }
    })
}

/// A component that takes the [`OriginalChildren`] of some preexistent DOM node and a continuation function `f`, and renders them into the DOM. Additionally, `f` is called on every child of the replaced element, to potentially "hydrate" them further.
#[component]
pub fn DomChildrenCont<
    V:IntoView+'static,
    R:FnOnce() -> V,
    F:Fn(&Element) -> Option<R>+'static+Send
>(orig:OriginalNode,cont:F,#[prop(optional)] on_load:Option<RwSignal<bool>>) -> impl IntoView {
    #[cfg(any(feature="csr",feature="hydrate"))]
    let inner = orig.inner.clone();
    orig.as_view(move |e| {
        #[cfg(any(feature="csr",feature="hydrate"))]
        {
            OriginalNode::do_children(&inner, e, 
                |c| dom::hydrate_node(c.clone(), &cont)
            );
            if let Some(on_load) = on_load { on_load.set(true); }
        }
    })
}

/// A component that renders a string of valid HTML, and then calls `f` on all the DOM nodes resulting from that to potentially "hydrate" them further.
#[component]
pub fn DomStringCont<
    V:IntoView+'static,
    R:FnOnce() -> V,
    F:Fn(&Element) -> Option<R>+'static
>(html:String,cont:F,#[prop(optional)] on_load:Option<RwSignal<bool>>) -> impl IntoView {
    let rf = NodeRef::<Span>::new();
    #[cfg(any(feature="csr",feature="hydrate"))]
    let mut cont = move |e| node::on_mount(e,move |e| {
        //leptos::logging::log!("Mounting {}",e.outer_html());
        OriginalNode::do_children(e, e, 
            |e| dom::hydrate_node(e, &cont)
        );
        if let Some(on_load) = on_load { on_load.set(true); }
    });
    rf.on_load(|e| {
        #[cfg(any(feature="csr",feature="hydrate"))]
        cont(e.into());
    });
    view!(<span node_ref=rf inner_html=html/>)
}

/// Like [`DomStringCont`], but using `<mrow>` instead of `<span>`.
#[component]
pub fn DomStringContMath<
    V:IntoView+'static,
    R:FnOnce() -> V,
    F:Fn(&Element) -> Option<R>+'static+Send
>(html:String,cont:F,#[prop(optional)] on_load:Option<RwSignal<bool>>) -> impl IntoView {
    let rf = NodeRef::<Mrow>::new();
    #[cfg(any(feature="csr",feature="hydrate"))]
    let mut cont = move |e| node::on_mount(e,move |e| {
        //leptos::logging::log!("Mounting {}",e.outer_html());
        OriginalNode::do_children(e, e, 
            |e| dom::hydrate_node(e, &cont)
        );
        if let Some(on_load) = on_load { on_load.set(true); }
    });
    rf.on_load(|e| {
        #[cfg(any(feature="csr",feature="hydrate"))]
        cont(e);
    });
    view!(<mrow node_ref=rf inner_html=html/>)
}


// need some check to not iterate over the entire body multiple times for some reason.
// I'm not sure why this is necessary, but it seems to be.
#[cfg(feature="csr")]
static DONE : std::sync::OnceLock<()> = std::sync::OnceLock::new();

/// Hydrates the entire DOM with leptos components, starting at the body.
/// 
/// `v` is a function that takes the [`OriginalChildren`] of the `<body>` (likely reinserting them somewhere) and returns some leptos view replacing the original children(!) of the body.
#[cfg(feature="csr")]
pub fn hydrate_body<N:IntoView>(
  v:impl FnOnce(OriginalNode) -> N + 'static
) {
  // make sure this only ever happens once.
  if DONE.get().is_some() {return}
  DONE.get_or_init(|| ());
  let document = leptos::tachys::dom::document();
  // We check that the DOM has been fully loaded
  let state = document.ready_state();
  let go = move || {
    let body = leptos::tachys::dom::body();
    let nd = leptos::tachys::dom::document().create_element("div").expect("Error creating div");
    while let Some(c) = body.child_nodes().get(0) {
      nd.append_child(&c).expect("Error appending child");
    };
    mount_to_body(move || v(nd.into()));
  };
  if state == "complete" || state == "interactive" {
    go();
  } else {
    use leptos::wasm_bindgen::JsCast;
    let fun = std::rc::Rc::new(std::cell::Cell::new(Some(go)));
    let closure = leptos::wasm_bindgen::closure::Closure::wrap(Box::new(move |_:leptos::web_sys::Event| {
      if let Some(f) = fun.take() {
        f()
      }
    }) as Box<dyn FnMut(_)>);
     document.add_event_listener_with_callback("DOMContentLoaded", closure.as_ref().unchecked_ref()).unwrap();
     closure.forget();
  }
}

// ------------------------------------------------------------


//static OBS_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/*

#[allow(unused_variables)]
fn replace_string_effect<V:IntoView+'static,E,F:Fn(&Element) -> Option<V>+'static+Clone>(rf:NodeRef<E>,conv:impl Fn(E::Output) -> Element + 'static,cont:F,signal:Option<RwSignal<bool>>)
where
    E: ElementType + 'static,
    E::Output:leptos::wasm_bindgen::JsCast + Clone + 'static {
  Effect::new(move |_| if let Some(node) = rf.get() {
    #[cfg(any(feature="csr",feature="hydrate"))]
    {
      let node = conv(node);
      dom::hydrate_children((*node).clone(), &cont);
      on_mount(node,move |node| {
        while let Some(mut c) = node.child_nodes().item(0) {
          if !node.insert_before_this(&mut c) {
            panic!("ERROR: Failed to insert child node!!");
          }
          let c = send_wrapper::SendWrapper::new(c);

          Owner::on_cleanup(move || {
            leptos::logging::warn!("Trying to cleanup {}",prettyprint(&*c));
            if let Some(p) = c.parent_element() {
              let _ = p.remove_child(&c);
            } else {
              leptos::logging::warn!("No parent found");
            }
          });
        }
        if let Some(signal) = signal {signal.set(true); }
      });
    }
  });
}


  */

#[cfg(any(feature="csr",feature="hydrate"))]
fn cleanup(node:leptos::web_sys::Node) {
    let c = SendWrapper::new(node);
    Owner::on_cleanup(move || {
        //leptos::logging::warn!("Trying to cleanup {}",prettyprint(&*c));
        if let Some(p) = c.parent_element() {
        let _ = p.remove_child(&c);
        } /*else {
        leptos::logging::warn!("No parent found");
        }*/
    });
}
/*
#[cfg(any(feature="csr",feature="hydrate"))]
fn prettyprint(node:&web_sys::Node) -> String {
  use leptos::wasm_bindgen::JsCast;
  if let Some(e) = node.dyn_ref::<Element>() {
    e.outer_html()
  } else if let Some(t) = node.dyn_ref::<web_sys::Text>() {
    t.data()
  } else if let Some(c) = node.dyn_ref::<web_sys::Comment>() {
    c.data()
  } else {
    node.to_string().as_string().expect("wut")
  }
}
   */