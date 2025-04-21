//! Transformer results
//! Building up types to contain the results of each layer at compile time requires a nested enum provided by [`Nested`]
//! However, the nested nature of this type is awkward to work with. So this module proveds the various `View*` types
//! which provide an idomatic way to access the results of each layer.
//! This module also provides various helpful type aliases and the `Get` and `GetMut` traits
//! which provide a way to access the results of each layer.

/// Nested result struct used to chain the results of each layer together, () is used to terminate the chain
/// This is similar in concept to how lists are implemented in functional programming
pub enum Nested<A, B> {
    /// Event for the most recent layer
    Some(A),
    /// Events for the rest of the layers
    Other(B),
}

/// Struct used the access the result at index 0
pub struct Index0;
/// Struct used the access the result at index 1
pub struct Index1;
/// Struct used the access the result at index 2
pub struct Index2;
/// Struct used the access the result at index 3
pub struct Index3;

/// Private index trait
trait Index {}

impl Index for Index0 {}
impl Index for Index1 {}
impl Index for Index2 {}
impl Index for Index3 {}

/// Trait to make it easy to access a certain result value of a certain type or at a certain index
#[allow(private_bounds)]
pub trait Get<T, I: Index> {
    /// Get the given result type, returns none if the result is not of the given type
    fn get(&self) -> Option<&T>;
}

/// rait to make it easy to access a certain result value of a certain type or at a certain index
#[allow(private_bounds)]
pub trait GetMut<T, I: Index> {
    /// Get the given result type, returns none if the result is not of the given type
    fn get_mut(&mut self) -> Option<&mut T>;
}

/// Type alias for a result from a single layer, the base entity will produce () as the result
pub type Nested1<A> = Nested<A, ()>;

impl<A> Get<A, Index0> for Nested1<A> {
    fn get(&self) -> Option<&A> {
        match self {
            Nested::Some(a) => Some(a),
            Nested::Other(_) => None,
        }
    }
}

impl<A> GetMut<A, Index0> for Nested1<A> {
    fn get_mut(&mut self) -> Option<&mut A> {
        match self {
            Nested::Some(a) => Some(a),
            Nested::Other(_) => None,
        }
    }
}

/// Type alias for a result from two layers
pub type Nested2<A, B> = Nested<A, Nested1<B>>;

impl<A, B> Get<A, Index0> for Nested2<A, B> {
    fn get(&self) -> Option<&A> {
        match self {
            Nested::Some(a) => Some(a),
            Nested::Other(_) => None,
        }
    }
}

impl<A, B> Get<B, Index1> for Nested2<A, B> {
    fn get(&self) -> Option<&B> {
        match self {
            Nested::Some(_) => None,
            Nested::Other(b) => b.get(),
        }
    }
}

impl<A, B> GetMut<A, Index0> for Nested2<A, B> {
    fn get_mut(&mut self) -> Option<&mut A> {
        match self {
            Nested::Some(a) => Some(a),
            Nested::Other(_) => None,
        }
    }
}

impl<A, B> GetMut<B, Index1> for Nested2<A, B> {
    fn get_mut(&mut self) -> Option<&mut B> {
        match self {
            Nested::Some(_) => None,
            Nested::Other(b) => b.get_mut(),
        }
    }
}

/// Type alias for a result from three layers
pub type Nested3<A, B, C> = Nested<A, Nested2<B, C>>;

impl<A, B, C> Get<A, Index0> for Nested3<A, B, C> {
    fn get(&self) -> Option<&A> {
        match self {
            Nested::Some(a) => Some(a),
            Nested::Other(_) => None,
        }
    }
}

impl<A, B, C> Get<B, Index1> for Nested3<A, B, C> {
    fn get(&self) -> Option<&B> {
        match self {
            Nested::Some(_) => None,
            Nested::Other(b) => b.get(),
        }
    }
}

impl<A, B, C> Get<C, Index2> for Nested3<A, B, C> {
    fn get(&self) -> Option<&C> {
        match self {
            Nested::Some(_) => None,
            Nested::Other(b) => b.get(),
        }
    }
}

impl<A, B, C> GetMut<A, Index0> for Nested3<A, B, C> {
    fn get_mut(&mut self) -> Option<&mut A> {
        match self {
            Nested::Some(a) => Some(a),
            Nested::Other(_) => None,
        }
    }
}

impl<A, B, C> GetMut<B, Index1> for Nested3<A, B, C> {
    fn get_mut(&mut self) -> Option<&mut B> {
        match self {
            Nested::Some(_) => None,
            Nested::Other(b) => b.get_mut(),
        }
    }
}

impl<A, B, C> GetMut<C, Index2> for Nested3<A, B, C> {
    fn get_mut(&mut self) -> Option<&mut C> {
        match self {
            Nested::Some(_) => None,
            Nested::Other(b) => b.get_mut(),
        }
    }
}
/// Type alias for a result from four layers
pub type Nested4<A, B, C, D> = Nested<A, Nested3<B, C, D>>;

impl<A, B, C, D> Get<A, Index0> for Nested4<A, B, C, D> {
    fn get(&self) -> Option<&A> {
        match self {
            Nested::Some(a) => Some(a),
            Nested::Other(_) => None,
        }
    }
}

impl<A, B, C, D> Get<B, Index1> for Nested4<A, B, C, D> {
    fn get(&self) -> Option<&B> {
        match self {
            Nested::Some(_) => None,
            Nested::Other(b) => b.get(),
        }
    }
}

impl<A, B, C, D> Get<C, Index2> for Nested4<A, B, C, D> {
    fn get(&self) -> Option<&C> {
        match self {
            Nested::Some(_) => None,
            Nested::Other(b) => b.get(),
        }
    }
}

impl<A, B, C, D> Get<D, Index3> for Nested4<A, B, C, D> {
    fn get(&self) -> Option<&D> {
        match self {
            Nested::Some(_) => None,
            Nested::Other(b) => b.get(),
        }
    }
}

impl<A, B, C, D> GetMut<A, Index0> for Nested4<A, B, C, D> {
    fn get_mut(&mut self) -> Option<&mut A> {
        match self {
            Nested::Some(a) => Some(a),
            Nested::Other(_) => None,
        }
    }
}

impl<A, B, C, D> GetMut<B, Index1> for Nested4<A, B, C, D> {
    fn get_mut(&mut self) -> Option<&mut B> {
        match self {
            Nested::Some(_) => None,
            Nested::Other(b) => b.get_mut(),
        }
    }
}

impl<A, B, C, D> GetMut<C, Index2> for Nested4<A, B, C, D> {
    fn get_mut(&mut self) -> Option<&mut C> {
        match self {
            Nested::Some(_) => None,
            Nested::Other(b) => b.get_mut(),
        }
    }
}

impl<A, B, C, D> GetMut<D, Index3> for Nested4<A, B, C, D> {
    fn get_mut(&mut self) -> Option<&mut D> {
        match self {
            Nested::Some(_) => None,
            Nested::Other(b) => b.get_mut(),
        }
    }
}

/// An idomatic, immutable view into the result of single layer
pub enum View1<'a, A> {
    /// Result for the previou
    Result0(&'a A),
}

/// An idomatic, mutable view into the result of single layer
pub enum ViewMut1<'a, A> {
    /// Result for the previous layer
    Result0(&'a mut A),
}

impl<'a, A> TryFrom<&'a Nested1<A>> for View1<'a, A> {
    type Error = ();

    fn try_from(value: &'a Nested1<A>) -> Result<Self, Self::Error> {
        match value {
            Nested::Some(a) => Ok(View1::Result0(a)),
            Nested::Other(_) => Err(()),
        }
    }
}

impl<'a, A> TryFrom<&'a mut Nested1<A>> for ViewMut1<'a, A> {
    type Error = ();

    fn try_from(value: &'a mut Nested1<A>) -> Result<Self, Self::Error> {
        match value {
            Nested::Some(a) => Ok(ViewMut1::Result0(a)),
            Nested::Other(_) => Err(()),
        }
    }
}

impl<'a, A> Get<A, Index0> for View1<'a, A> {
    fn get(&self) -> Option<&A> {
        match self {
            View1::Result0(a) => Some(a),
        }
    }
}

impl<'a, A> GetMut<A, Index0> for ViewMut1<'a, A> {
    fn get_mut(&mut self) -> Option<&mut A> {
        match self {
            ViewMut1::Result0(a) => Some(a),
        }
    }
}

/// An idomatic, immutable view into the result of two layers
pub enum View2<'a, A, B> {
    /// Result for the first layer
    Result0(&'a A),
    /// Result for the second layer
    Result1(&'a B),
}

/// An idomatic, mutable view into the result of two layers
pub enum ViewMut2<'a, A, B> {
    /// Result for the first layer
    Result0(&'a mut A),
    /// Result for the second layer
    Result1(&'a mut B),
}

impl<'a, A, B> TryFrom<&'a Nested2<A, B>> for View2<'a, A, B> {
    type Error = ();

    fn try_from(value: &'a Nested2<A, B>) -> Result<Self, Self::Error> {
        match value {
            Nested::Some(a) => Ok(View2::Result0(a)),
            Nested::Other(Nested::Some(b)) => Ok(View2::Result1(b)),
            _ => Err(()),
        }
    }
}

impl<'a, A, B> TryFrom<&'a mut Nested2<A, B>> for ViewMut2<'a, A, B> {
    type Error = ();

    fn try_from(value: &'a mut Nested2<A, B>) -> Result<Self, Self::Error> {
        match value {
            Nested::Some(a) => Ok(ViewMut2::Result0(a)),
            Nested::Other(Nested::Some(b)) => Ok(ViewMut2::Result1(b)),
            _ => Err(()),
        }
    }
}

impl<'a, A, B> Get<A, Index0> for View2<'a, A, B> {
    fn get(&self) -> Option<&A> {
        match self {
            View2::Result0(a) => Some(a),
            View2::Result1(_) => None,
        }
    }
}

impl<'a, A, B> Get<B, Index1> for View2<'a, A, B> {
    fn get(&self) -> Option<&B> {
        match self {
            View2::Result0(_) => None,
            View2::Result1(b) => Some(b),
        }
    }
}

impl<'a, A, B> GetMut<A, Index0> for ViewMut2<'a, A, B> {
    fn get_mut(&mut self) -> Option<&mut A> {
        match self {
            ViewMut2::Result0(a) => Some(a),
            ViewMut2::Result1(_) => None,
        }
    }
}

impl<'a, A, B> GetMut<B, Index1> for ViewMut2<'a, A, B> {
    fn get_mut(&mut self) -> Option<&mut B> {
        match self {
            ViewMut2::Result0(_) => None,
            ViewMut2::Result1(b) => Some(b),
        }
    }
}

/// An idomatic, immutable view into the result of three layers
pub enum View3<'a, A, B, C> {
    /// Result for the first layer
    Result0(&'a A),
    /// Result for the second layer
    Result1(&'a B),
    /// Result for the third layer
    Result2(&'a C),
}

/// An idomatic, mutable view into the result of three layers
pub enum ViewMut3<'a, A, B, C> {
    /// Result for the first layer
    Result0(&'a mut A),
    /// Result for the second layer
    Result1(&'a mut B),
    /// Result for the third layer
    Result2(&'a mut C),
}

impl<'a, A, B, C> TryFrom<&'a Nested3<A, B, C>> for View3<'a, A, B, C> {
    type Error = ();

    fn try_from(value: &'a Nested3<A, B, C>) -> Result<Self, Self::Error> {
        match value {
            Nested::Some(a) => Ok(View3::Result0(a)),
            Nested::Other(Nested::Some(b)) => Ok(View3::Result1(b)),
            Nested::Other(Nested::Other(Nested::Some(c))) => Ok(View3::Result2(c)),
            _ => Err(()),
        }
    }
}

impl<'a, A, B, C> TryFrom<&'a mut Nested3<A, B, C>> for ViewMut3<'a, A, B, C> {
    type Error = ();

    fn try_from(value: &'a mut Nested3<A, B, C>) -> Result<Self, Self::Error> {
        match value {
            Nested::Some(a) => Ok(ViewMut3::Result0(a)),
            Nested::Other(Nested::Some(b)) => Ok(ViewMut3::Result1(b)),
            Nested::Other(Nested::Other(Nested::Some(c))) => Ok(ViewMut3::Result2(c)),
            _ => Err(()),
        }
    }
}

impl<'a, A, B, C> Get<A, Index0> for View3<'a, A, B, C> {
    fn get(&self) -> Option<&A> {
        match self {
            View3::Result0(a) => Some(a),
            _ => None,
        }
    }
}

impl<'a, A, B, C> Get<B, Index1> for View3<'a, A, B, C> {
    fn get(&self) -> Option<&B> {
        match self {
            View3::Result1(b) => Some(b),
            _ => None,
        }
    }
}

impl<'a, A, B, C> Get<C, Index2> for View3<'a, A, B, C> {
    fn get(&self) -> Option<&C> {
        match self {
            View3::Result2(c) => Some(c),
            _ => None,
        }
    }
}

impl<'a, A, B, C> GetMut<A, Index0> for ViewMut3<'a, A, B, C> {
    fn get_mut(&mut self) -> Option<&mut A> {
        match self {
            ViewMut3::Result0(a) => Some(a),
            _ => None,
        }
    }
}

impl<'a, A, B, C> GetMut<B, Index1> for ViewMut3<'a, A, B, C> {
    fn get_mut(&mut self) -> Option<&mut B> {
        match self {
            ViewMut3::Result1(b) => Some(b),
            _ => None,
        }
    }
}

impl<'a, A, B, C> GetMut<C, Index2> for ViewMut3<'a, A, B, C> {
    fn get_mut(&mut self) -> Option<&mut C> {
        match self {
            ViewMut3::Result2(c) => Some(c),
            _ => None,
        }
    }
}

/// An idomatic, immutable view into the result of four layers
pub enum View4<'a, A, B, C, D> {
    /// Result for the first layer
    Result0(&'a A),
    /// Result for the second layer
    Result1(&'a B),
    /// Result for the third layer
    Result2(&'a C),
    /// Result for the fourth layer
    Result3(&'a D),
}

/// An idomatic, mutable view into the result of four layers
pub enum ViewMut4<'a, A, B, C, D> {
    /// Result for the first layer
    Result0(&'a mut A),
    /// Result for the second layer
    Result1(&'a mut B),
    /// Result for the third layer
    Result2(&'a mut C),
    /// Result for the fourth layer
    Result3(&'a mut D),
}

impl<'a, A, B, C, D> TryFrom<&'a Nested4<A, B, C, D>> for View4<'a, A, B, C, D> {
    type Error = ();

    fn try_from(value: &'a Nested4<A, B, C, D>) -> Result<Self, Self::Error> {
        match value {
            Nested::Some(a) => Ok(View4::Result0(a)),
            Nested::Other(Nested::Some(b)) => Ok(View4::Result1(b)),
            Nested::Other(Nested::Other(Nested::Some(c))) => Ok(View4::Result2(c)),
            Nested::Other(Nested::Other(Nested::Other(Nested::Some(d)))) => Ok(View4::Result3(d)),
            _ => Err(()),
        }
    }
}

impl<'a, A, B, C, D> TryFrom<&'a mut Nested4<A, B, C, D>> for ViewMut4<'a, A, B, C, D> {
    type Error = ();

    fn try_from(value: &'a mut Nested4<A, B, C, D>) -> Result<Self, Self::Error> {
        match value {
            Nested::Some(a) => Ok(ViewMut4::Result0(a)),
            Nested::Other(Nested::Some(b)) => Ok(ViewMut4::Result1(b)),
            Nested::Other(Nested::Other(Nested::Some(c))) => Ok(ViewMut4::Result2(c)),
            Nested::Other(Nested::Other(Nested::Other(Nested::Some(d)))) => Ok(ViewMut4::Result3(d)),
            _ => Err(()),
        }
    }
}

impl<'a, A, B, C, D> Get<A, Index0> for View4<'a, A, B, C, D> {
    fn get(&self) -> Option<&A> {
        match self {
            View4::Result0(a) => Some(a),
            _ => None,
        }
    }
}

impl<'a, A, B, C, D> Get<B, Index1> for View4<'a, A, B, C, D> {
    fn get(&self) -> Option<&B> {
        match self {
            View4::Result1(b) => Some(b),
            _ => None,
        }
    }
}

impl<'a, A, B, C, D> Get<C, Index2> for View4<'a, A, B, C, D> {
    fn get(&self) -> Option<&C> {
        match self {
            View4::Result2(c) => Some(c),
            _ => None,
        }
    }
}

impl<'a, A, B, C, D> Get<D, Index3> for View4<'a, A, B, C, D> {
    fn get(&self) -> Option<&D> {
        match self {
            View4::Result3(d) => Some(d),
            _ => None,
        }
    }
}

impl<'a, A, B, C, D> GetMut<A, Index0> for ViewMut4<'a, A, B, C, D> {
    fn get_mut(&mut self) -> Option<&mut A> {
        match self {
            ViewMut4::Result0(a) => Some(a),
            _ => None,
        }
    }
}

impl<'a, A, B, C, D> GetMut<B, Index1> for ViewMut4<'a, A, B, C, D> {
    fn get_mut(&mut self) -> Option<&mut B> {
        match self {
            ViewMut4::Result1(b) => Some(b),
            _ => None,
        }
    }
}

impl<'a, A, B, C, D> GetMut<C, Index2> for ViewMut4<'a, A, B, C, D> {
    fn get_mut(&mut self) -> Option<&mut C> {
        match self {
            ViewMut4::Result2(c) => Some(c),
            _ => None,
        }
    }
}

impl<'a, A, B, C, D> GetMut<D, Index3> for ViewMut4<'a, A, B, C, D> {
    fn get_mut(&mut self) -> Option<&mut D> {
        match self {
            ViewMut4::Result3(d) => Some(d),
            _ => None,
        }
    }
}
