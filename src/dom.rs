use web_sys::Element;

#[cfg(any(feature="csr",feature="hydrate"))]
use web_sys::Node;
#[cfg(any(feature="csr",feature="hydrate"))]
use leptos::{prelude::{Dom, Mountable, Owner, Render}, tachys::view::any_view::AnyView, IntoView};
#[cfg(any(feature="csr",feature="hydrate"))]
use wasm_bindgen::JsCast;


/// Represents the original children some node in the DOM had, to be used in the [`DomChildren`](super::DomChildren), [`DomChildrenCont`](super::DomChildrenCont) and [`DomStringCont`](super::DomStringCont) components.
pub struct OriginalChildren(
  // Server side, this is just an empty struct, since there's no DOM anyway.
  #[cfg(any(feature="csr",feature="hydrate"))]
  pub(crate) send_wrapper::SendWrapper<Element>
);
impl OriginalChildren {
  pub fn new(_e:&Element) -> Self {
    #[cfg(any(feature="csr",feature="hydrate"))]
    {
      // it's annoying that this seems to be how to take all the children...
      /*
      let mut vec = Vec::new();
      while let Some(c) = _e.child_nodes().item(0) {
        let _ = _e.remove_child(&c);
        vec.push(c);
      }
       */
      OriginalChildren(send_wrapper::SendWrapper::new(_e.clone()))
    }
    #[cfg(not(any(feature="csr",feature="hydrate")))]
    { OriginalChildren() }
  }
  #[cfg(any(feature="csr",feature="hydrate"))]
  pub(crate) fn clone_children(&self) -> Vec<Node> {
    assert!(self.0.valid());
    let mut vec = Vec::new();
    let mut i = 0;
    while let Some(c) = self.0.child_nodes().item(i) {
      vec.push(c);i+=1;
    }
    vec
  }
}

// Iterated over the node and its children (DFS) and replaces elements via the given function.
#[cfg(any(feature="csr",feature="hydrate"))]
pub fn hydrate_node(node:Node,replace:&impl Fn(&Element) -> Option<AnyView<Dom>>) {
  // Check node returns a new index if it replaced the node, otherwise None.
  if check_node(node.clone(),0,replace).is_some() {return}
  // Non-recursive DOM iteration
  let mut current = node;
  let mut index = 0u32;
  let mut stack : Vec<(Node,u32)> = Vec::new();
  loop {
    if let Some(c) = current.child_nodes().item(index) {
      // Check node returns a new index if it replaced the node, otherwise None.
      if let Some(skip) = check_node(c.clone(),index,replace) {
        index = skip;
        continue;
      }
      if c.has_child_nodes() {
        let old = std::mem::replace(&mut current,c);
        stack.push((old,index + 1));
        index = 0;
      } else { index += 1;}
    } else if let Some((old,idx)) = stack.pop() {
        current = old;
        index = idx;
    } else { break; }
  }
}

// Actually replaces nodes:
#[cfg(any(feature="csr",feature="hydrate"))]
fn check_node(node:Node,mut start:u32,replace:&impl Fn(&Element) -> Option<AnyView<Dom>>) -> Option<u32> {
  if let Ok(e) = node.dyn_into::<Element>() {
    if let Some(v) = replace(&e) {
      // This is mostly copied from leptos::mount_to_body and related methods
      let mut r = v.into_view().build();
      e.insert_before_this(&mut r);
      // we need to keep the state alive. My buest guess is to hand it over to the owner to clean it up when it deems it necessary.
      let r = send_wrapper::SendWrapper::new(r);
      Owner::on_cleanup(move|| {drop(r)});
      // remove the old element and return the index at which to continue iteration
      let p = e.parent_node().unwrap();
      while let Some(c) = p.child_nodes().item(start) {
        if c == *e {
          break
        }
        start += 1;
      }
      e.remove();
      return Some(start);
    }
  }
  None
}