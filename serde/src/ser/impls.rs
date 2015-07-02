use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;
use std::path;
use std::rc::Rc;
use std::sync::Arc;

use super::{
    Serialize,
    Serializer,
    SeqVisitor,
    MapVisitor,
};

///////////////////////////////////////////////////////////////////////////////

macro_rules! impl_visit {
    ($ty:ty, $method:ident) => {
        impl Serialize for $ty {
            #[inline]
            fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
                where S: Serializer,
            {
                serializer.$method(*self)
            }
        }
    }
}

impl_visit!(bool, visit_bool);
impl_visit!(isize, visit_isize);
impl_visit!(i8, visit_i8);
impl_visit!(i16, visit_i16);
impl_visit!(i32, visit_i32);
impl_visit!(i64, visit_i64);
impl_visit!(usize, visit_usize);
impl_visit!(u8, visit_u8);
impl_visit!(u16, visit_u16);
impl_visit!(u32, visit_u32);
impl_visit!(u64, visit_u64);
impl_visit!(f32, visit_f32);
impl_visit!(f64, visit_f64);
impl_visit!(char, visit_char);

///////////////////////////////////////////////////////////////////////////////

impl Serialize for str {
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        serializer.visit_str(self)
    }
}

impl Serialize for String {
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        (&self[..]).serialize(serializer)
    }
}

///////////////////////////////////////////////////////////////////////////////

impl<T> Serialize for Option<T> where T: Serialize {
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        match *self {
            Some(ref value) => serializer.visit_some(value),
            None => serializer.visit_none(),
        }
    }
}

///////////////////////////////////////////////////////////////////////////////

pub struct SeqIteratorVisitor<Iter> {
    iter: Iter,
    len: Option<usize>,
}

impl<T, Iter> SeqIteratorVisitor<Iter>
    where Iter: Iterator<Item=T>
{
    #[inline]
    pub fn new(iter: Iter, len: Option<usize>) -> SeqIteratorVisitor<Iter> {
        SeqIteratorVisitor {
            iter: iter,
            len: len,
        }
    }
}

impl<T, Iter> SeqVisitor for SeqIteratorVisitor<Iter>
    where T: Serialize,
          Iter: Iterator<Item=T>,
{
    #[inline]
    fn visit<S>(&mut self, serializer: &mut S) -> Result<Option<()>, S::Error>
        where S: Serializer,
    {
        match self.iter.next() {
            Some(value) => {
                let value = try!(serializer.visit_seq_elt(value));
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    #[inline]
    fn len(&self) -> Option<usize> {
        self.len
    }
}

///////////////////////////////////////////////////////////////////////////////

impl<T> Serialize for [T]
    where T: Serialize,
{
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        serializer.visit_seq(SeqIteratorVisitor::new(self.iter(), Some(self.len())))
    }
}

macro_rules! array_impls {
    ($len:expr) => {
        impl<T> Serialize for [T; $len] where T: Serialize {
            #[inline]
            fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
                where S: Serializer,
            {
                serializer.visit_seq(SeqIteratorVisitor::new(self.iter(), Some($len)))
            }
        }
    }
}

array_impls!(0);
array_impls!(1);
array_impls!(2);
array_impls!(3);
array_impls!(4);
array_impls!(5);
array_impls!(6);
array_impls!(7);
array_impls!(8);
array_impls!(9);
array_impls!(10);
array_impls!(11);
array_impls!(12);
array_impls!(13);
array_impls!(14);
array_impls!(15);
array_impls!(16);
array_impls!(17);
array_impls!(18);
array_impls!(19);
array_impls!(20);
array_impls!(21);
array_impls!(22);
array_impls!(23);
array_impls!(24);
array_impls!(25);
array_impls!(26);
array_impls!(27);
array_impls!(28);
array_impls!(29);
array_impls!(30);
array_impls!(31);
array_impls!(32);

impl<T> Serialize for Vec<T> where T: Serialize {
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        (&self[..]).serialize(serializer)
    }
}

impl<T> Serialize for BTreeSet<T>
    where T: Serialize + Ord,
{
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        serializer.visit_seq(SeqIteratorVisitor::new(self.iter(), Some(self.len())))
    }
}

impl<T> Serialize for HashSet<T>
    where T: Serialize + Eq + Hash,
{
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        serializer.visit_seq(SeqIteratorVisitor::new(self.iter(), Some(self.len())))
    }
}

///////////////////////////////////////////////////////////////////////////////

impl Serialize for () {
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        serializer.visit_unit()
    }
}

///////////////////////////////////////////////////////////////////////////////

// FIXME(rust #19630) Remove this work-around
macro_rules! e {
    ($e:expr) => { $e }
}

macro_rules! tuple_impls {
    ($(
        $TupleVisitor:ident ($len:expr, $($T:ident),+) {
            $($state:pat => $idx:tt,)+
        }
    )+) => {
        $(
            pub struct $TupleVisitor<'a, $($T: 'a),+> {
                tuple: &'a ($($T,)+),
                state: u8,
            }

            impl<'a, $($T: 'a),+> $TupleVisitor<'a, $($T),+> {
                pub fn new(tuple: &'a ($($T,)+)) -> $TupleVisitor<'a, $($T),+> {
                    $TupleVisitor {
                        tuple: tuple,
                        state: 0,
                    }
                }
            }

            impl<'a, $($T),+> SeqVisitor for $TupleVisitor<'a, $($T),+>
                where $($T: Serialize),+
            {
                fn visit<S>(&mut self, serializer: &mut S) -> Result<Option<()>, S::Error>
                    where S: Serializer,
                {
                    match self.state {
                        $(
                            $state => {
                                self.state += 1;
                                Ok(Some(try!(serializer.visit_seq_elt(&e!(self.tuple.$idx)))))
                            }
                        )+
                        _ => {
                            Ok(None)
                        }
                    }
                }

                fn len(&self) -> Option<usize> {
                    Some($len)
                }
            }

            impl<$($T),+> Serialize for ($($T,)+)
                where $($T: Serialize),+
            {
                #[inline]
                fn serialize<S: Serializer>(&self, serializer: &mut S) -> Result<(), S::Error> {
                    serializer.visit_tuple($TupleVisitor::new(self))
                }
            }
        )+
    }
}

tuple_impls! {
    TupleVisitor1 (1, T0) {
        0 => 0,
    }
    TupleVisitor2 (2, T0, T1) {
        0 => 0,
        1 => 1,
    }
    TupleVisitor3 (3, T0, T1, T2) {
        0 => 0,
        1 => 1,
        2 => 2,
    }
    TupleVisitor4 (4, T0, T1, T2, T3) {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
    }
    TupleVisitor5 (5, T0, T1, T2, T3, T4) {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
    }
    TupleVisitor6 (6, T0, T1, T2, T3, T4, T5) {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 5,
    }
    TupleVisitor7 (7, T0, T1, T2, T3, T4, T5, T6) {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 5,
        6 => 6,
    }
    TupleVisitor8 (8, T0, T1, T2, T3, T4, T5, T6, T7) {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 5,
        6 => 6,
        7 => 7,
    }
    TupleVisitor9 (9, T0, T1, T2, T3, T4, T5, T6, T7, T8) {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 5,
        6 => 6,
        7 => 7,
        8 => 8,
    }
    TupleVisitor10 (10, T0, T1, T2, T3, T4, T5, T6, T7, T8, T9) {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 5,
        6 => 6,
        7 => 7,
        8 => 8,
        9 => 9,
    }
    TupleVisitor11 (11, T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10) {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 5,
        6 => 6,
        7 => 7,
        8 => 8,
        9 => 9,
        10 => 10,
    }
    TupleVisitor12 (12, T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11) {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 5,
        6 => 6,
        7 => 7,
        8 => 8,
        9 => 9,
        10 => 10,
        11 => 11,
    }
}

///////////////////////////////////////////////////////////////////////////////

pub struct MapIteratorVisitor<Iter> {
    iter: Iter,
    len: Option<usize>,
}

impl<K, V, Iter> MapIteratorVisitor<Iter>
    where Iter: Iterator<Item=(K, V)>
{
    #[inline]
    pub fn new(iter: Iter, len: Option<usize>) -> MapIteratorVisitor<Iter> {
        MapIteratorVisitor {
            iter: iter,
            len: len,
        }
    }
}

impl<K, V, I> MapVisitor for MapIteratorVisitor<I>
    where K: Serialize,
          V: Serialize,
          I: Iterator<Item=(K, V)>,
{
    #[inline]
    fn visit<S>(&mut self, serializer: &mut S) -> Result<Option<()>, S::Error>
        where S: Serializer,
    {
        match self.iter.next() {
            Some((key, value)) => {
                let value = try!(serializer.visit_map_elt(key, value));
                Ok(Some(value))
            }
            None => Ok(None)
        }
    }

    #[inline]
    fn len(&self) -> Option<usize> {
        self.len
    }
}

///////////////////////////////////////////////////////////////////////////////

impl<K, V> Serialize for BTreeMap<K, V>
    where K: Serialize + Ord,
          V: Serialize,
{
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        serializer.visit_map(MapIteratorVisitor::new(self.iter(), Some(self.len())))
    }
}

impl<K, V> Serialize for HashMap<K, V>
    where K: Serialize + Eq + Hash,
          V: Serialize,
{
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        serializer.visit_map(MapIteratorVisitor::new(self.iter(), Some(self.len())))
    }
}

// FIXME: `VecMap` is unstable.
/*
impl<V> Serialize for VecMap<V>
    where V: Serialize,
{
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        serializer.visit_map(MapIteratorVisitor::new(self.iter(), Some(self.len())))
    }
}
*/

///////////////////////////////////////////////////////////////////////////////

impl<'a, T: ?Sized> Serialize for &'a T where T: Serialize {
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        (**self).serialize(serializer)
    }
}

impl<'a, T: ?Sized> Serialize for &'a mut T where T: Serialize {
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        (**self).serialize(serializer)
    }
}

impl<T: ?Sized> Serialize for Box<T> where T: Serialize {
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        (**self).serialize(serializer)
    }
}

impl<T> Serialize for Rc<T> where T: Serialize, {
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        (**self).serialize(serializer)
    }
}

impl<T> Serialize for Arc<T> where T: Serialize, {
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        (**self).serialize(serializer)
    }
}

///////////////////////////////////////////////////////////////////////////////

impl Serialize for path::Path {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        self.to_str().unwrap().serialize(serializer)
    }
}

impl Serialize for path::PathBuf {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer,
    {
        self.to_str().unwrap().serialize(serializer)
    }
}
