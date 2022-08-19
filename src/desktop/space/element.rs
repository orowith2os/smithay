use crate::{
    backend::renderer::{ImportAll, Renderer},
    desktop::{space::*, utils as desktop_utils},
    utils::{Logical, Physical, Point, Rectangle, Scale},
};
use std::hash::Hash;

/// Indicates default values for some zindexs inside smithay
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum RenderZindex {
    /// WlrLayer::Background default zindex
    Background = 10,
    /// WlrLayer::Bottom default zindex
    Bottom = 20,
    /// Default zindex for Windows
    Shell = 30,
    /// WlrLayer::Top default zindex
    Top = 40,
    /// Default Layer for RenderElements
    Overlay = 60,
}

impl From<RenderZindex> for u8 {
    fn from(idx: RenderZindex) -> u8 {
        idx as u8
    }
}

impl From<RenderZindex> for Option<u8> {
    fn from(idx: RenderZindex) -> Option<u8> {
        Some(idx as u8)
    }
}

/// Trait for a space element
pub trait SpaceElement: IsAlive {
    /// Gets the geometry of this element
    fn geometry(&self) -> Rectangle<i32, Logical> {
        self.bbox()
    }
    fn bbox(&self) -> Rectangle<i32, Logical>;
    fn input_region(&self, point: &Point<f64, Logical>) -> Option<Point<i32, Logical>>;
    /// Gets the z-index of this element on the specified space
    fn z_index(&self) -> u8 {
        RenderZindex::Overlay as u8
    }

    fn set_activate(&self, activated: bool);
    fn output_enter(&self, output: &Output);
    fn output_leave(&self, output: &Output);
}

impl<T: SpaceElement> SpaceElement for &T {
    fn geometry(&self) -> Rectangle<i32, Logical> {
        SpaceElement::geometry(*self)
    }
    fn bbox(&self) -> Rectangle<i32, Logical> {
        SpaceElement::bbox(*self)
    }
    fn input_region(&self, point: &Point<f64, Logical>) -> Option<Point<i32, Logical>> {
        SpaceElement::input_region(*self, point)
    }
    fn z_index(&self) -> u8 {
        SpaceElement::z_index(*self)
    }

    fn set_activate(&self, activated: bool) {
        SpaceElement::set_activate(*self, activated)
    }
    fn output_enter(&self, output: &Output) {
        SpaceElement::output_enter(*self, output)
    }
    fn output_leave(&self, output: &Output) {
        SpaceElement::output_leave(*self, output)
    }
}

space_elements! {
    pub SpaceElements<'a, E>;
    Layer=LayerSurface,
    Element=&'a E,
}

impl<'a, R, E> AsRenderElements<R> for SpaceElements<'a, E>
where
    R: Renderer + ImportAll,
    <R as Renderer>::TextureId: Texture + 'static,
    E: AsRenderElements<R>,
    <E as AsRenderElements<R>>::RenderElement: 'a,
    SpaceRenderElements<'a, R, <E as AsRenderElements<R>>::RenderElement>:
        From<<E as AsRenderElements<R>>::RenderElement> + From<WaylandSurfaceRenderElement>,
{
    type RenderElement = SpaceRenderElements<'a, R, <E as AsRenderElements<R>>::RenderElement>;

    fn render_elements<C: From<Self::RenderElement>>(
        &self,
        location: Point<i32, Physical>,
        scale: Scale<f64>,
    ) -> Vec<C> {
        match &self {
            SpaceElements::Layer(layer) => {
                AsRenderElements::<R>::render_elements::<Self::RenderElement>(layer, location, scale)
                    .into_iter()
                    .map(C::from)
                    .collect()
            }
            SpaceElements::Element(element) => element
                .render_elements::<Self::RenderElement>(location, scale)
                .into_iter()
                .map(C::from)
                .collect(),
            _ => unreachable!(),
        }
    }
}
/// A custom surface tree
#[derive(Debug)]
pub struct SurfaceTree {
    location: Point<i32, Logical>,
    surface: WlSurface,
}

impl SurfaceTree {
    /// Create a surface tree from a surface
    pub fn from_surface(surface: &WlSurface, location: impl Into<Point<i32, Logical>>) -> Self {
        SurfaceTree {
            location: location.into(),
            surface: surface.clone(),
        }
    }
}

impl IsAlive for SurfaceTree {
    fn alive(&self) -> bool {
        self.surface.alive()
    }
}

impl SpaceElement for SurfaceTree {
    fn geometry(&self) -> Rectangle<i32, Logical> {
        self.bbox()
    }

    fn bbox(&self) -> Rectangle<i32, Logical> {
        desktop_utils::bbox_from_surface_tree(&self.surface, self.location)
    }

    fn input_region(&self, point: &Point<f64, Logical>) -> Option<Point<i32, Logical>> {
        desktop_utils::under_from_surface_tree(&self.surface, *point, (0, 0), WindowSurfaceType::ALL)
            .map(|(s, point)| point)
    }

    fn set_activate(&self, activated: bool) {}
    fn output_enter(&self, output: &Output) {
        with_surface_tree_downward(
            &self.surface,
            (),
            |_, _, _| TraversalAction::DoChildren(()),
            |wl_surface, _, _| {
                output.enter(wl_surface);
            },
            |_, _, _| true,
        );
    }
    fn output_leave(&self, output: &Output) {
        with_surface_tree_downward(
            &self.surface,
            (),
            |_, _, _| TraversalAction::DoChildren(()),
            |wl_surface, _, _| {
                output.leave(wl_surface);
            },
            |_, _, _| true,
        );
    }
}

impl<R> AsRenderElements<R> for SurfaceTree
where
    R: Renderer + ImportAll,
    <R as Renderer>::TextureId: 'static,
{
    type RenderElement = WaylandSurfaceRenderElement;

    fn render_elements<C: From<WaylandSurfaceRenderElement>>(
        &self,
        location: Point<i32, Physical>,
        scale: Scale<f64>,
    ) -> Vec<C> {
        crate::backend::renderer::element::surface::render_elements_from_surface_tree(
            &self.surface,
            location,
            scale,
        )
    }
}

#[macro_export]
#[doc(hidden)]
macro_rules! space_elements_internal {
    (@enum $(#[$attr:meta])* $vis:vis $name:ident<$lt:lifetime, $custom:ident>; $($(#[$meta:meta])* $body:ident=$field:ty),* $(,)?) => {
        $(#[$attr])*
        $vis enum $name<$lt, $custom> {
            $(
                $(
                    #[$meta]
                )*
                $body($field)
            ),*,
            #[doc(hidden)]
            _GenericCatcher(std::convert::Infallible),
        }
    };
    (@enum $(#[$attr:meta])* $vis:vis $name:ident<$custom:ident>; $($(#[$meta:meta])* $body:ident=$field:ty),* $(,)?) => {
        $(#[$attr])*
        $vis enum $name<$custom> {
            $(
                $(
                    #[$meta]
                )*
                $body($field)
            ),*,
            #[doc(hidden)]
            _GenericCatcher(std::convert::Infallible),
        }
    };
    (@enum $(#[$attr:meta])* $vis:vis $name:ident<$lt:lifetime>; $($(#[$meta:meta])* $body:ident=$field:ty),* $(,)?) => {
        $(#[$attr])*
        $vis enum $name<$lt> {
            $(
                $(
                    #[$meta]
                )*
                $body($field)
            ),*,
            #[doc(hidden)]
            _GenericCatcher(std::convert::Infallible),
        }
    };
    (@enum $(#[$attr:meta])* $vis:vis $name:ident; $($(#[$meta:meta])* $body:ident=$field:ty),* $(,)?) => {
        $(#[$attr])*
        $vis enum $name {
            $(
                $(
                    #[$meta]
                )*
                $body($field)
            ),*,
            #[doc(hidden)]
            _GenericCatcher(std::convert::Infallible),
        }
    };
    (@call $name:ident; $($x:ident),*) => {
        $crate::desktop::space::SpaceElement::$name($($x),*)
    };
    (@alive $($(#[$meta:meta])* $body:ident=$field:ty),* $(,)?) => {
        fn alive(&self) -> bool {
            match self {
                $(
                    #[allow(unused_doc_comments)]
                    $(
                        #[$meta]
                    )*
                    Self::$body(x) => $crate::utils::IsAlive::alive(x)
                ),*,
                Self::_GenericCatcher(_) => unreachable!(),
            }
        }
    };
    (@body $($(#[$meta:meta])* $body:ident=$field:ty),* $(,)?) => {
        fn geometry(&self) -> $crate::utils::Rectangle<i32, $crate::utils::Logical> {
            match self {
                $(
                    #[allow(unused_doc_comments)]
                    $(
                        #[$meta]
                    )*
                    Self::$body(x) => $crate::space_elements_internal!(@call geometry; x)
                ),*,
                Self::_GenericCatcher(_) => unreachable!(),
            }
        }
        fn bbox(&self) -> $crate::utils::Rectangle<i32, $crate::utils::Logical> {
            match self {
                $(
                    #[allow(unused_doc_comments)]
                    $(
                        #[$meta]
                    )*
                    Self::$body(x) => $crate::space_elements_internal!(@call bbox; x)
                ),*,
                Self::_GenericCatcher(_) => unreachable!(),
            }
        }
        fn input_region(&self, point: &$crate::utils::Point<f64, $crate::utils::Logical>) -> Option<$crate::utils::Point<i32, $crate::utils::Logical>> {
            match self {
                $(
                    #[allow(unused_doc_comments)]
                    $(
                        #[$meta]
                    )*
                    Self::$body(x) => $crate::space_elements_internal!(@call input_region; x, point)
                ),*,
                Self::_GenericCatcher(_) => unreachable!(),
            }
        }

        fn z_index(&self) -> u8 {
            match self {
                $(
                    #[allow(unused_doc_comments)]
                    $(
                        #[$meta]
                    )*
                    Self::$body(x) => $crate::space_elements_internal!(@call z_index; x)
                ),*,
                Self::_GenericCatcher(_) => unreachable!(),
            }
        }

        fn set_activate(&self, activated: bool) {
            match self {
                $(
                    #[allow(unused_doc_comments)]
                    $(
                        #[$meta]
                    )*
                    Self::$body(x) => $crate::space_elements_internal!(@call set_activate; x, activated)
                ),*,
                Self::_GenericCatcher(_) => unreachable!(),
            }
        }
        fn output_enter(&self, output: &$crate::wayland::output::Output) {
            match self {
                $(
                    #[allow(unused_doc_comments)]
                    $(
                        #[$meta]
                    )*
                    Self::$body(x) => $crate::space_elements_internal!(@call output_enter; x, output)
                ),*,
                Self::_GenericCatcher(_) => unreachable!(),
            }
        }
        fn output_leave(&self, output: &$crate::wayland::output::Output) {
            match self {
                $(
                    #[allow(unused_doc_comments)]
                    $(
                        #[$meta]
                    )*
                    Self::$body(x) => $crate::space_elements_internal!(@call output_leave; x, output)
                ),*,
                Self::_GenericCatcher(_) => unreachable!(),
            }
        }
    };
    (@impl $name:ident<$lt:lifetime>; $($tail:tt)*) => {
        impl<$lt> $crate::desktop::space::SpaceElement for $name<$lt>
        {
            $crate::space_elements_internal!(@body $($tail)*);
        }
        impl<$lt> $crate::utils::IsAlive for $name<$lt>
        {
            $crate::space_elements_internal!(@alive $($tail)*);
        }
    };
    (@impl $name:ident<$lt:lifetime, $custom:ident>; $($tail:tt)*) => {
        impl<$lt, $custom> $crate::desktop::space::SpaceElement for $name<$lt, $custom>
        where
            $custom: $crate::desktop::space::SpaceElement,
        {
            $crate::space_elements_internal!(@body $($tail)*);
        }
        impl<$lt, $custom> $crate::utils::IsAlive for $name<$lt, $custom>
        where
            $custom: $crate::utils::IsAlive,
        {
            $crate::space_elements_internal!(@alive $($tail)*);
        }
    };
    (@impl $name:ident<$custom:ident>; $($tail:tt)*) => {
        impl<$custom> $crate::desktop::space::SpaceElement for $name<$custom>
        where
            $custom: $crate::desktop::space::SpaceElement,
        {
            $crate::space_elements_internal!(@body $($tail)*);
        }
        impl<$custom> $crate::utils::IsAlive for $name<$custom>
        where
            $custom: $crate::utils::IsAlive,
        {
            $crate::space_elements_internal!(@alive $($tail)*);
        }
    };
    (@impl $name:ident; $($tail:tt)*) => {
        impl $crate::desktop::space::SpaceElement for $name
        {
            $crate::space_elements_internal!(@body $($tail)*);
        }
        impl $crate::utils::IsAlive for $name
        {
            $crate::space_elements_internal!(@alive $($tail)*);
        }
    };
}

/// TODO: Docs
#[macro_export]
macro_rules! space_elements {
    ($(#[$attr:meta])* $vis:vis $name:ident<$lt:lifetime, $custom:ident>; $($tail:tt)*) => {
        $crate::space_elements_internal!(@enum $(#[$attr])* $vis $name<$lt, $custom>; $($tail)*);
        $crate::space_elements_internal!(@impl $name<$lt, $custom>; $($tail)*);
    };
    ($(#[$attr:meta])* $vis:vis $name:ident<$custom:ident>; $($tail:tt)*) => {
        $crate::space_elements_internal!(@enum $(#[$attr])* $vis $name<$custom>; $($tail)*);
        $crate::space_elements_internal!(@impl $name<$custom>; $($tail)*);
    };
    ($(#[$attr:meta])* $vis:vis $name:ident<$lt:lifetime>; $($tail:tt)*) => {
        $crate::space_elements_internal!(@enum $(#[$attr])* $vis $name<$lt>; $($tail)*);
        $crate::space_elements_internal!(@impl $name<$lt>; $($tail)*);
    };
    ($(#[$attr:meta])* $vis:vis $name:ident; $($tail:tt)*) => {
        $crate::space_elements_internal!(@enum $(#[$attr])* $vis $name; $($tail)*);
        $crate::space_elements_internal!(@impl $name; $($tail)*);
    };
}

pub use space_elements;

#[cfg(test)]
mod tests {
    use super::SurfaceTree;
    use crate::desktop::{LayerSurface, Window};

    space_elements! {
        /// Some test space elements
        pub TestSpaceElements;
        /// A complete surface tree
        SurfaceTree=SurfaceTree,
    }

    space_elements! {
        /// Some test space elements
        pub TestSpaceElements2<'a>;
        /// A complete surface tree
        Window=&'a Window,
        /// A layer surface
        LayerSurface=&'a LayerSurface,
    }

    space_elements! {
        /// Some test space elements
        pub TestSpaceElements5<'a, C>;
        /// A complete surface tree
        SurfaceTree=SurfaceTree,
        Custom=&'a C,
    }

    space_elements! {
        /// Some test space elements
        pub TestSpaceElements7<C>;
        /// A complete surface tree
        SurfaceTree=SurfaceTree,
        Custom=C,
    }
}
