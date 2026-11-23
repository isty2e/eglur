use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::record_reservation::RecordReservation;
use crate::{CanonicalTarget, DefId, ExternalDefId, IdentityOverflow, SubstId, TypeId};

macro_rules! primitive_types {
    ($($variant:ident => $spelling:literal),+ $(,)?) => {
        /// Concrete language-intrinsic scalar and singleton types.
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum PrimitiveType {
            $($variant),+
        }

        impl PrimitiveType {
            pub const ALL: [Self; [$(stringify!($variant)),+].len()] = [$(Self::$variant),+];

            /// Returns the canonical source spelling of this primitive.
            #[must_use]
            pub const fn spelling(self) -> &'static str {
                match self {
                    $(Self::$variant => $spelling),+
                }
            }

            #[must_use]
            pub fn from_spelling(spelling: &str) -> Option<Self> {
                match spelling {
                    $($spelling => Some(Self::$variant)),+,
                    _ => None,
                }
            }
        }
    };
}

primitive_types! {
    Unit => "Unit",
    Bool => "Bool",
    Str => "Str",
    I8 => "i8",
    I16 => "i16",
    I32 => "i32",
    I64 => "i64",
    U8 => "u8",
    U16 => "u16",
    U32 => "u32",
    U64 => "u64",
    F16 => "f16",
    Bf16 => "bf16",
    F32 => "f32",
    F64 => "f64",
    Complex64 => "complex64",
    Complex128 => "complex128",
    Nat => "Nat",
    PositiveNat => "PositiveNat",
}

impl PrimitiveType {
    /// Selects the binary numeric dtype; this does not convert operand values.
    /// Domain scalars and nonnumeric types have no promotion. Bool + Bool selects u8.
    #[must_use]
    pub const fn promote_numeric(self, other: Self) -> Option<Self> {
        let (Some(left), Some(right)) = (self.numeric_precedence(), other.numeric_precedence())
        else {
            return None;
        };
        if matches!((self, other), (Self::Bool, Self::Bool)) {
            Some(Self::U8)
        } else if left >= right {
            Some(self)
        } else {
            Some(other)
        }
    }

    #[must_use]
    pub const fn is_numeric(self) -> bool {
        self.numeric_precedence().is_some()
    }

    const fn numeric_precedence(self) -> Option<u8> {
        match self {
            Self::Bool => Some(0),
            Self::U8 => Some(1),
            Self::I8 => Some(2),
            Self::U16 => Some(3),
            Self::I16 => Some(4),
            Self::U32 => Some(5),
            Self::I32 => Some(6),
            Self::U64 => Some(7),
            Self::I64 => Some(8),
            Self::F16 => Some(9),
            Self::Bf16 => Some(10),
            Self::F32 => Some(11),
            Self::F64 => Some(12),
            Self::Complex64 => Some(13),
            Self::Complex128 => Some(14),
            Self::Unit | Self::Str | Self::Nat | Self::PositiveNat => None,
        }
    }
}

pub(crate) fn is_reserved_type_name(name: &str) -> bool {
    matches!(
        name,
        "Opt" | "Res" | "Own" | "Box" | "Fn" | "Union" | "Never" | "Self"
    )
}

/// Language-intrinsic nominal constructor identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntrinsicNominal {
    Own,
    Box,
    Opt,
    Res,
}

impl IntrinsicNominal {
    #[must_use]
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::Own => "Own",
            Self::Box => "Box",
            Self::Opt => "Opt",
            Self::Res => "Res",
        }
    }

    #[must_use]
    pub const fn arity(self) -> usize {
        match self {
            Self::Own | Self::Box | Self::Opt => 1,
            Self::Res => 2,
        }
    }

    #[must_use]
    pub fn from_spelling(spelling: &str) -> Option<Self> {
        match spelling {
            "Own" => Some(Self::Own),
            "Box" => Some(Self::Box),
            "Opt" => Some(Self::Opt),
            "Res" => Some(Self::Res),
            _ => None,
        }
    }
}

/// Identity of a nominal type constructor, excluding transparent aliases.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NominalHead {
    Intrinsic(IntrinsicNominal),
    Declaration(CanonicalTarget),
}

/// Stable generic-parameter identity within one compilation graph.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GenericParameterId {
    Source(DefId),
    Trusted { owner: ExternalDefId, index: u32 },
}

/// Mutability encoded by a canonical reference type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReferenceMutability {
    Shared,
    Mutable,
}

/// Mutability encoded by a canonical raw pointer type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RawPointerMutability {
    Const,
    Mutable,
}

/// A complete nominal trait application, kept outside ordinary value types.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TraitRef {
    declaration: CanonicalTarget,
    substitution: SubstId,
}

impl TraitRef {
    #[must_use]
    pub(crate) const fn new(declaration: CanonicalTarget, substitution: SubstId) -> Self {
        Self {
            declaration,
            substitution,
        }
    }

    #[must_use]
    pub const fn declaration(&self) -> &CanonicalTarget {
        &self.declaration
    }

    #[must_use]
    pub const fn substitution(&self) -> SubstId {
        self.substitution
    }
}

/// Unnormalized associated type whose declaring bound remains symbolic.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssociatedProjection {
    base: TypeId,
    trait_ref: TraitRef,
    associated: CanonicalTarget,
}

impl AssociatedProjection {
    #[must_use]
    pub(crate) const fn new(
        base: TypeId,
        trait_ref: TraitRef,
        associated: CanonicalTarget,
    ) -> Self {
        Self {
            base,
            trait_ref,
            associated,
        }
    }

    #[must_use]
    pub const fn base(&self) -> TypeId {
        self.base
    }

    #[must_use]
    pub const fn trait_ref(&self) -> &TraitRef {
        &self.trait_ref
    }

    #[must_use]
    pub const fn associated(&self) -> &CanonicalTarget {
        &self.associated
    }
}

/// Alias-expanded canonical type algebra.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeKind {
    Never,
    Primitive(PrimitiveType),
    Nominal {
        head: NominalHead,
        substitution: SubstId,
    },
    GenericParameter(GenericParameterId),
    SelfType(CanonicalTarget),
    Reference {
        mutability: ReferenceMutability,
        referent: TypeId,
    },
    RawPointer {
        mutability: RawPointerMutability,
        referent: TypeId,
    },
    Tuple(Box<[TypeId]>),
    Callable {
        parameters: Box<[TypeId]>,
        result: TypeId,
    },
    ErrorUnion(Box<[TypeId]>),
    AssociatedProjection(AssociatedProjection),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PayloadDependency<'a> {
    Type(TypeId),
    Projection(&'a AssociatedProjection),
}

/// Canonical ownership-bearing root recognized by v0.1 dataflow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipRoot {
    Direct { payload: TypeId },
    Optional { payload: TypeId },
    Result { payload: TypeId, error: TypeId },
}

/// Interned canonical type and complete-substitution graph.
pub struct CanonicalTypeStore {
    never: TypeId,
    types: Vec<TypeKind>,
    may_need_generic_substitution: Vec<bool>,
    ownership_presence: Vec<Option<bool>>,
    type_ids: BTreeMap<TypeKind, TypeId>,
    substitutions: Vec<Box<[TypeId]>>,
    substitution_ownership_presence: Vec<Option<bool>>,
    substitution_ids: BTreeMap<Box<[TypeId]>, SubstId>,
    nominal_arities: BTreeMap<NominalHead, usize>,
}

impl CanonicalTypeStore {
    pub fn new() -> Result<Self, TypeStoreError> {
        let never = TypeId::from_index(0)?;
        let mut store = Self {
            never,
            types: Vec::new(),
            may_need_generic_substitution: Vec::new(),
            ownership_presence: Vec::new(),
            type_ids: BTreeMap::new(),
            substitutions: Vec::new(),
            substitution_ownership_presence: Vec::new(),
            substitution_ids: BTreeMap::new(),
            nominal_arities: BTreeMap::new(),
        };
        let interned = store.intern_type(TypeKind::Never)?;
        debug_assert_eq!(interned, never);
        store.intern_substitution([])?;
        for intrinsic in [
            IntrinsicNominal::Own,
            IntrinsicNominal::Box,
            IntrinsicNominal::Opt,
            IntrinsicNominal::Res,
        ] {
            store.register_nominal_head(NominalHead::Intrinsic(intrinsic), intrinsic.arity())?;
        }
        Ok(store)
    }

    /// Returns the graph-local identity of the intrinsic uninhabited type.
    #[must_use]
    pub const fn never(&self) -> TypeId {
        self.never
    }

    /// Registers one nominal constructor before applications are interned.
    pub fn register_nominal_head(
        &mut self,
        head: NominalHead,
        arity: usize,
    ) -> Result<(), TypeStoreError> {
        if let NominalHead::Declaration(CanonicalTarget::Primitive(primitive)) = head {
            return Err(TypeStoreError::PrimitiveNominalHead(primitive));
        }
        if let NominalHead::Declaration(CanonicalTarget::Intrinsic(intrinsic)) = head {
            return Err(TypeStoreError::IntrinsicDeclarationHead(intrinsic));
        }
        if let Some(&existing) = self.nominal_arities.get(&head) {
            if existing != arity {
                return Err(TypeStoreError::ConflictingNominalArity {
                    head,
                    existing,
                    incoming: arity,
                });
            }
            return Ok(());
        }
        self.nominal_arities.insert(head, arity);
        Ok(())
    }

    #[must_use]
    pub fn nominal_arity(&self, head: &NominalHead) -> Option<usize> {
        self.nominal_arities.get(head).copied()
    }

    pub fn primitive(&mut self, primitive: PrimitiveType) -> Result<TypeId, TypeStoreError> {
        self.intern_type(TypeKind::Primitive(primitive))
    }

    pub fn generic_parameter(
        &mut self,
        parameter: GenericParameterId,
    ) -> Result<TypeId, TypeStoreError> {
        self.intern_type(TypeKind::GenericParameter(parameter))
    }

    pub fn self_type(&mut self, owner: CanonicalTarget) -> Result<TypeId, TypeStoreError> {
        self.intern_type(TypeKind::SelfType(owner))
    }

    pub fn nominal(
        &mut self,
        head: NominalHead,
        arguments: impl IntoIterator<Item = TypeId>,
    ) -> Result<TypeId, TypeStoreError> {
        let arguments = arguments.into_iter().collect::<Vec<_>>();
        let expected = self
            .nominal_arity(&head)
            .ok_or_else(|| TypeStoreError::UnregisteredNominalHead(head.clone()))?;
        if arguments.len() != expected {
            return Err(TypeStoreError::NominalArity {
                head,
                expected,
                actual: arguments.len(),
            });
        }
        let substitution = self.intern_substitution(arguments)?;
        self.intern_type(TypeKind::Nominal { head, substitution })
    }

    pub fn reference(
        &mut self,
        mutability: ReferenceMutability,
        referent: TypeId,
    ) -> Result<TypeId, TypeStoreError> {
        self.require_type(referent)?;
        self.intern_type(TypeKind::Reference {
            mutability,
            referent,
        })
    }

    pub fn raw_pointer(
        &mut self,
        mutability: RawPointerMutability,
        referent: TypeId,
    ) -> Result<TypeId, TypeStoreError> {
        self.require_type(referent)?;
        self.intern_type(TypeKind::RawPointer {
            mutability,
            referent,
        })
    }

    pub fn tuple(
        &mut self,
        elements: impl IntoIterator<Item = TypeId>,
    ) -> Result<TypeId, TypeStoreError> {
        let elements = elements.into_iter().collect::<Vec<_>>();
        self.require_types(&elements)?;
        self.intern_type(TypeKind::Tuple(elements.into_boxed_slice()))
    }

    pub fn callable(
        &mut self,
        parameters: impl IntoIterator<Item = TypeId>,
        result: TypeId,
    ) -> Result<TypeId, TypeStoreError> {
        let parameters = parameters.into_iter().collect::<Vec<_>>();
        self.require_types(&parameters)?;
        self.require_type(result)?;
        self.intern_type(TypeKind::Callable {
            parameters: parameters.into_boxed_slice(),
            result,
        })
    }

    /// Interns a flattened, duplicate-free error union without impossible members.
    pub fn error_union(
        &mut self,
        members: impl IntoIterator<Item = TypeId>,
    ) -> Result<TypeId, TypeStoreError> {
        let mut pending = members.into_iter().collect::<Vec<_>>();
        if pending.is_empty() {
            return Err(TypeStoreError::EmptyUnion);
        }
        self.require_types(&pending)?;
        let mut normalized = BTreeSet::new();
        while let Some(member) = pending.pop() {
            match self.kind(member)? {
                TypeKind::ErrorUnion(nested) => pending.extend(nested.iter().copied()),
                TypeKind::Never => {}
                _ => {
                    normalized.insert(member);
                }
            }
        }
        if normalized.is_empty() {
            return Ok(self.never());
        }
        if normalized.len() == 1 {
            return Ok(*normalized
                .first()
                .expect("one-member normalized union has one type"));
        }
        self.intern_type(TypeKind::ErrorUnion(
            normalized
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ))
    }

    pub fn associated_projection(
        &mut self,
        projection: AssociatedProjection,
    ) -> Result<TypeId, TypeStoreError> {
        self.require_type(projection.base)?;
        self.substitution(projection.trait_ref.substitution)?;
        self.intern_type(TypeKind::AssociatedProjection(projection))
    }

    /// Unknown-length streams must be collected before entering the interner.
    ///
    /// ```compile_fail,E0277
    /// use eglur_sema::{CanonicalTypeStore, PrimitiveType};
    ///
    /// let mut types = CanonicalTypeStore::new().unwrap();
    /// let integer = types.primitive(PrimitiveType::I32).unwrap();
    /// let arguments = [integer].into_iter().filter(|_| true);
    /// types.intern_substitution(arguments).unwrap();
    /// ```
    pub fn intern_substitution(
        &mut self,
        arguments: impl IntoIterator<Item = TypeId, IntoIter: ExactSizeIterator>,
    ) -> Result<SubstId, TypeStoreError> {
        let arguments = arguments.into_iter().collect::<Vec<_>>();
        self.require_types(&arguments)?;
        let arguments = arguments.into_boxed_slice();
        if let Some(&id) = self.substitution_ids.get(&arguments) {
            return Ok(id);
        }
        let id = SubstId::from_index(self.substitutions.len())?;
        let ownership_presence = self.ownership_presence_for_types(arguments.iter().copied());

        let records = RecordReservation::for_append(&self.substitutions);
        let ownership = RecordReservation::for_append(&self.substitution_ownership_presence);

        records.reserve(&mut self.substitutions);
        ownership.reserve(&mut self.substitution_ownership_presence);

        self.substitution_ids.insert(arguments.clone(), id);
        self.substitutions.push(arguments);
        self.substitution_ownership_presence
            .push(ownership_presence);
        Ok(id)
    }

    pub fn kind(&self, id: TypeId) -> Result<&TypeKind, TypeStoreError> {
        #[cfg(test)]
        ownership_presence_tests::observe_kind_read();
        self.types
            .get(id.index())
            .ok_or(TypeStoreError::UnknownType(id))
    }

    pub fn substitution(&self, id: SubstId) -> Result<&[TypeId], TypeStoreError> {
        self.substitutions
            .get(id.index())
            .map(AsRef::as_ref)
            .ok_or(TypeStoreError::UnknownSubstitution(id))
    }

    #[must_use]
    pub fn type_count(&self) -> usize {
        self.types.len()
    }

    pub fn types(&self) -> impl ExactSizeIterator<Item = (TypeId, &TypeKind)> {
        self.types.iter().enumerate().map(|(index, kind)| {
            (
                TypeId::from_index(index).expect("stored type index fits TypeId"),
                kind,
            )
        })
    }

    #[must_use]
    pub fn substitution_count(&self) -> usize {
        self.substitutions.len()
    }

    /// Applies a finite generic-parameter mapping to an interned type graph.
    pub fn substitute(
        &mut self,
        root: TypeId,
        replacements: &BTreeMap<GenericParameterId, TypeId>,
    ) -> Result<TypeId, TypeStoreError> {
        self.require_type(root)?;
        self.require_types(replacements.values())?;
        if replacements.is_empty() || !self.may_need_generic_substitution[root.index()] {
            return Ok(root);
        }

        self.rewrite(root, rewrite::LeafRewrite::Generic(replacements))
    }

    /// Replaces one declaration-owned `Self` throughout an interned type graph.
    pub(crate) fn replace_self_type(
        &mut self,
        root: TypeId,
        owner: &CanonicalTarget,
        replacement: TypeId,
    ) -> Result<TypeId, TypeStoreError> {
        self.require_type(root)?;
        self.require_type(replacement)?;
        self.rewrite(root, rewrite::LeafRewrite::OwnedSelf { owner, replacement })
    }

    pub fn contains_reference(&self, root: TypeId) -> Result<bool, TypeStoreError> {
        self.contains(root, |kind| matches!(kind, TypeKind::Reference { .. }))
    }

    pub fn contains_ownership(&self, root: TypeId) -> Result<bool, TypeStoreError> {
        self.require_type(root)?;
        if let Some(present) = self.ownership_presence[root.index()] {
            return Ok(present);
        }

        // Incomplete private metadata retains the original fallible traversal.
        self.contains(root, |kind| {
            matches!(
                kind,
                TypeKind::Nominal {
                    head: NominalHead::Intrinsic(IntrinsicNominal::Own),
                    ..
                }
            )
        })
    }

    pub fn contains_raw_pointer(&self, root: TypeId) -> Result<bool, TypeStoreError> {
        self.contains(root, |kind| matches!(kind, TypeKind::RawPointer { .. }))
    }

    pub fn contains_associated_projection(&self, root: TypeId) -> Result<bool, TypeStoreError> {
        self.contains(root, |kind| {
            matches!(kind, TypeKind::AssociatedProjection(_))
        })
    }

    /// Checks the closed v0.1 directional value-assignability relation.
    pub(crate) fn is_assignable(
        &self,
        actual: TypeId,
        expected: TypeId,
    ) -> Result<bool, TypeStoreError> {
        if actual == expected || actual == self.never() {
            return Ok(true);
        }
        let actual_kind = self.kind(actual)?.clone();
        let expected_kind = self.kind(expected)?.clone();
        match (actual_kind, expected_kind) {
            (TypeKind::ErrorUnion(sources), TypeKind::ErrorUnion(targets)) => {
                for source in &sources {
                    let mut covered = false;
                    for target in &targets {
                        if self.is_assignable(*source, *target)? {
                            covered = true;
                            break;
                        }
                    }
                    if !covered {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (_, TypeKind::ErrorUnion(targets)) => {
                for target in &targets {
                    if self.is_assignable(actual, *target)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            (TypeKind::ErrorUnion(_), _) => Ok(false),
            (
                TypeKind::Nominal {
                    head: NominalHead::Intrinsic(IntrinsicNominal::Res),
                    substitution: actual_substitution,
                },
                TypeKind::Nominal {
                    head: NominalHead::Intrinsic(IntrinsicNominal::Res),
                    substitution: expected_substitution,
                },
            ) => {
                let actual_arguments = self.substitution(actual_substitution)?;
                let expected_arguments = self.substitution(expected_substitution)?;
                Ok(actual_arguments[0] == expected_arguments[0]
                    && self.is_assignable(actual_arguments[1], expected_arguments[1])?)
            }
            (
                TypeKind::Reference {
                    mutability: ReferenceMutability::Mutable,
                    referent: actual_referent,
                },
                TypeKind::Reference {
                    mutability: ReferenceMutability::Shared,
                    referent: expected_referent,
                },
            ) => Ok(actual_referent == expected_referent),
            (
                TypeKind::RawPointer {
                    mutability: RawPointerMutability::Mutable,
                    referent: actual_referent,
                },
                TypeKind::RawPointer {
                    mutability: RawPointerMutability::Const,
                    referent: expected_referent,
                },
            ) => Ok(actual_referent == expected_referent),
            (
                TypeKind::Callable {
                    parameters: actual_parameters,
                    result: actual_result,
                },
                TypeKind::Callable {
                    parameters: expected_parameters,
                    result: expected_result,
                },
            ) => {
                if actual_parameters.len() != expected_parameters.len() {
                    return Ok(false);
                }
                for (expected_parameter, actual_parameter) in
                    expected_parameters.iter().zip(actual_parameters.iter())
                {
                    if !self.is_assignable(*expected_parameter, *actual_parameter)? {
                        return Ok(false);
                    }
                }
                self.is_assignable(actual_result, expected_result)
            }
            _ => Ok(false),
        }
    }

    /// Immediate payload dependencies; projection identity metadata remains terminal.
    pub(crate) fn payload_dependencies(
        &self,
        root: TypeId,
    ) -> Result<impl Iterator<Item = PayloadDependency<'_>>, TypeStoreError> {
        self.payload_dependencies_for_kind(self.kind(root)?)
    }

    fn payload_dependencies_for_kind<'a>(
        &'a self,
        kind: &'a TypeKind,
    ) -> Result<impl Iterator<Item = PayloadDependency<'a>>, TypeStoreError> {
        let (children, terminal): (&[TypeId], _) = match kind {
            TypeKind::Never
            | TypeKind::Primitive(_)
            | TypeKind::GenericParameter(_)
            | TypeKind::SelfType(_) => (&[], None),
            TypeKind::Nominal { substitution, .. } => (self.substitution(*substitution)?, None),
            TypeKind::Reference { referent, .. } | TypeKind::RawPointer { referent, .. } => {
                (std::slice::from_ref(referent), None)
            }
            TypeKind::Tuple(elements) | TypeKind::ErrorUnion(elements) => (elements, None),
            TypeKind::Callable { parameters, result } => {
                (parameters, Some(PayloadDependency::Type(*result)))
            }
            TypeKind::AssociatedProjection(projection) => {
                (&[], Some(PayloadDependency::Projection(projection)))
            }
        };
        Ok(children
            .iter()
            .copied()
            .map(PayloadDependency::Type)
            .chain(terminal))
    }

    pub fn ownership_root(&self, root: TypeId) -> Result<Option<OwnershipRoot>, TypeStoreError> {
        let TypeKind::Nominal { head, substitution } = self.kind(root)? else {
            return Ok(None);
        };
        let arguments = self.substitution(*substitution)?;
        match head {
            NominalHead::Intrinsic(IntrinsicNominal::Own) => Ok(Some(OwnershipRoot::Direct {
                payload: arguments[0],
            })),
            NominalHead::Intrinsic(carrier @ (IntrinsicNominal::Opt | IntrinsicNominal::Res)) => {
                let TypeKind::Nominal {
                    head: NominalHead::Intrinsic(IntrinsicNominal::Own),
                    substitution: inner,
                } = self.kind(arguments[0])?
                else {
                    return Ok(None);
                };
                let payload = self.substitution(*inner)?[0];
                Ok(Some(if *carrier == IntrinsicNominal::Opt {
                    OwnershipRoot::Optional { payload }
                } else {
                    OwnershipRoot::Result {
                        payload,
                        error: arguments[1],
                    }
                }))
            }
            _ => Ok(None),
        }
    }

    // False proves identity for generic substitution, including projection metadata.
    // Immutable constructor operands already have their own admission facts.
    fn kind_needs_generic_substitution(&self, kind: &TypeKind) -> bool {
        let child = |id: TypeId| {
            #[cfg(test)]
            substitution_tests::observe_child_fact();
            self.may_need_generic_substitution
                .get(id.index())
                .copied()
                .unwrap_or(true)
        };
        match kind {
            TypeKind::GenericParameter(_) => true,
            TypeKind::Never | TypeKind::Primitive(_) | TypeKind::SelfType(_) => false,
            TypeKind::Nominal { substitution, .. } => self
                .substitution(*substitution)
                .map_or(true, |arguments| arguments.iter().any(|&id| child(id))),
            TypeKind::Reference { referent, .. } | TypeKind::RawPointer { referent, .. } => {
                child(*referent)
            }
            TypeKind::Tuple(elements) | TypeKind::ErrorUnion(elements) => {
                elements.iter().any(|&id| child(id))
            }
            TypeKind::Callable { parameters, result } => {
                child(*result) || parameters.iter().any(|&id| child(id))
            }
            TypeKind::AssociatedProjection(projection) => {
                child(projection.base)
                    || self
                        .substitution(projection.trait_ref.substitution)
                        .map_or(true, |arguments| arguments.iter().any(|&id| child(id)))
            }
        }
    }

    fn intern_type(&mut self, kind: TypeKind) -> Result<TypeId, TypeStoreError> {
        if let Some(&id) = self.type_ids.get(&kind) {
            return Ok(id);
        }
        let id = TypeId::from_index(self.types.len())?;
        let may_need_generic_substitution = self.kind_needs_generic_substitution(&kind);
        let ownership_presence = self.kind_ownership_presence(&kind);

        let records = RecordReservation::for_append(&self.types);
        let generic = RecordReservation::for_append(&self.may_need_generic_substitution);
        let ownership = RecordReservation::for_append(&self.ownership_presence);

        records.reserve(&mut self.types);
        generic.reserve(&mut self.may_need_generic_substitution);
        ownership.reserve(&mut self.ownership_presence);

        self.type_ids.insert(kind.clone(), id);
        self.types.push(kind);
        self.may_need_generic_substitution
            .push(may_need_generic_substitution);
        self.ownership_presence.push(ownership_presence);
        #[cfg(test)]
        substitution_tests::observe_admission();
        Ok(id)
    }

    fn kind_ownership_presence(&self, kind: &TypeKind) -> Option<bool> {
        match kind {
            TypeKind::Nominal {
                head: NominalHead::Intrinsic(IntrinsicNominal::Own),
                ..
            } => Some(true),
            TypeKind::Nominal { substitution, .. } => self
                .substitution_ownership_presence
                .get(substitution.index())
                .copied()
                .flatten(),
            _ => self.ownership_presence_for_types(
                self.payload_dependencies_for_kind(kind)
                    .ok()?
                    .filter_map(|dependency| match dependency {
                        PayloadDependency::Type(child) => Some(child),
                        PayloadDependency::Projection(_) => None,
                    }),
            ),
        }
    }

    fn ownership_presence_for_types(&self, children: impl Iterator<Item = TypeId>) -> Option<bool> {
        let mut present = false;
        for child in children {
            present |= self
                .ownership_presence
                .get(child.index())
                .copied()
                .flatten()?;
        }
        Some(present)
    }

    fn require_type(&self, id: TypeId) -> Result<(), TypeStoreError> {
        self.kind(id).map(|_| ())
    }

    fn require_types(
        &self,
        ids: impl IntoIterator<Item = impl std::borrow::Borrow<TypeId>>,
    ) -> Result<(), TypeStoreError> {
        for id in ids {
            self.require_type(*id.borrow())?;
        }
        Ok(())
    }

    fn contains(
        &self,
        root: TypeId,
        predicate: impl Fn(&TypeKind) -> bool,
    ) -> Result<bool, TypeStoreError> {
        self.require_type(root)?;
        let mut pending = vec![root];
        let mut seen = BTreeSet::new();
        while let Some(current) = pending.pop() {
            if !seen.insert(current) {
                continue;
            }
            let kind = self.kind(current)?;
            if predicate(kind) {
                return Ok(true);
            }
            pending.extend(
                self.payload_dependencies_for_kind(kind)?.filter_map(
                    |dependency| match dependency {
                        PayloadDependency::Type(child) => Some(child),
                        PayloadDependency::Projection(_) => None,
                    },
                ),
            );
        }
        Ok(false)
    }
}

impl Default for CanonicalTypeStore {
    fn default() -> Self {
        Self::new().expect("empty canonical type store fits graph identity capacity")
    }
}

/// Canonical type-store construction or lookup failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeStoreError {
    IdentityOverflow(IdentityOverflow),
    UnknownType(TypeId),
    UnknownSubstitution(SubstId),
    UnregisteredNominalHead(NominalHead),
    PrimitiveNominalHead(PrimitiveType),
    IntrinsicDeclarationHead(IntrinsicNominal),
    ConflictingNominalArity {
        head: NominalHead,
        existing: usize,
        incoming: usize,
    },
    NominalArity {
        head: NominalHead,
        expected: usize,
        actual: usize,
    },
    EmptyUnion,
}

impl fmt::Display for TypeStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityOverflow(error) => error.fmt(formatter),
            Self::UnknownType(id) => write!(formatter, "unknown canonical type {}", id.get()),
            Self::UnknownSubstitution(id) => {
                write!(formatter, "unknown canonical substitution {}", id.get())
            }
            Self::UnregisteredNominalHead(head) => {
                write!(formatter, "unregistered nominal type head {head:?}")
            }
            Self::PrimitiveNominalHead(primitive) => {
                write!(
                    formatter,
                    "primitive {} is not a nominal type head",
                    primitive.spelling()
                )
            }
            Self::IntrinsicDeclarationHead(intrinsic) => write!(
                formatter,
                "intrinsic {} must use its canonical nominal head",
                intrinsic.spelling()
            ),
            Self::ConflictingNominalArity {
                head,
                existing,
                incoming,
            } => write!(
                formatter,
                "nominal type head {head:?} has conflicting arities {existing} and {incoming}"
            ),
            Self::NominalArity {
                head,
                expected,
                actual,
            } => write!(
                formatter,
                "nominal type head {head:?} expects {expected} arguments, found {actual}"
            ),
            Self::EmptyUnion => formatter.write_str("canonical error union cannot be empty"),
        }
    }
}

impl std::error::Error for TypeStoreError {}

impl From<IdentityOverflow> for TypeStoreError {
    fn from(error: IdentityOverflow) -> Self {
        Self::IdentityOverflow(error)
    }
}

#[cfg(test)]
mod record_growth_tests;

#[cfg(test)]
mod ownership_presence_tests;
mod rewrite;

#[cfg(test)]
mod rewrite_tests;

#[cfg(test)]
mod substitution_inputs_tests;

#[cfg(test)]
mod substitution_tests;

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::BTreeMap;

    use super::*;

    thread_local! {
        pub(super) static SUBSTITUTION_READS: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) fn observe_substitution_read() {
        SUBSTITUTION_READS.set(SUBSTITUTION_READS.get() + 1);
    }

    fn external(key: &str) -> CanonicalTarget {
        CanonicalTarget::External(ExternalDefId::new(key).unwrap())
    }

    #[test]
    fn primitive_spellings_roundtrip_without_alias_or_case_normalization() {
        let mut spellings = BTreeSet::new();
        for primitive in PrimitiveType::ALL {
            assert!(spellings.insert(primitive.spelling()));
            assert_eq!(
                PrimitiveType::from_spelling(primitive.spelling()),
                Some(primitive)
            );
        }
        for unknown in ["", "bool", "I32", "Int32", "std::prelude::i32"] {
            assert_eq!(PrimitiveType::from_spelling(unknown), None);
        }
    }

    #[test]
    fn primitive_bindings_cannot_be_admitted_as_nominal_heads() {
        let mut store = CanonicalTypeStore::new().unwrap();
        for primitive in PrimitiveType::ALL {
            let head = NominalHead::Declaration(CanonicalTarget::Primitive(primitive));
            assert!(store.register_nominal_head(head.clone(), 0).is_err());
            assert_eq!(store.nominal_arity(&head), None);
            assert!(store.nominal(head, []).is_err());

            let ty = store.primitive(primitive).unwrap();
            assert_eq!(store.kind(ty).unwrap(), &TypeKind::Primitive(primitive));
        }
    }

    #[test]
    fn intrinsic_bindings_cannot_be_wrapped_in_declaration_nominal_heads() {
        let mut store = CanonicalTypeStore::new().unwrap();
        let scalar = store.primitive(PrimitiveType::I64).unwrap();
        for intrinsic in [
            IntrinsicNominal::Own,
            IntrinsicNominal::Box,
            IntrinsicNominal::Opt,
            IntrinsicNominal::Res,
        ] {
            let duplicate = NominalHead::Declaration(CanonicalTarget::Intrinsic(intrinsic));
            assert_eq!(
                store.register_nominal_head(duplicate.clone(), intrinsic.arity()),
                Err(TypeStoreError::IntrinsicDeclarationHead(intrinsic))
            );
            assert_eq!(store.nominal_arity(&duplicate), None);
            let arguments = vec![scalar; intrinsic.arity()];
            assert!(store.nominal(duplicate, arguments.iter().copied()).is_err());
            let ty = store
                .nominal(NominalHead::Intrinsic(intrinsic), arguments)
                .unwrap();
            assert!(
                matches!(store.kind(ty).unwrap(), TypeKind::Nominal { head: NominalHead::Intrinsic(found), .. } if *found == intrinsic)
            );
        }
    }

    #[test]
    fn structural_types_and_complete_substitutions_are_interned() {
        let mut store = CanonicalTypeStore::new().unwrap();
        let item = store.primitive(PrimitiveType::I64).unwrap();
        let first = store
            .nominal(NominalHead::Intrinsic(IntrinsicNominal::Opt), [item])
            .unwrap();
        let second = store
            .nominal(NominalHead::Intrinsic(IntrinsicNominal::Opt), [item])
            .unwrap();

        assert_eq!(first, second);
        assert_eq!(store.substitution_count(), 2);
        assert!(matches!(
            store.nominal(NominalHead::Intrinsic(IntrinsicNominal::Res), [item]),
            Err(TypeStoreError::NominalArity {
                expected: 2,
                actual: 1,
                ..
            })
        ));
    }

    #[test]
    fn never_has_one_allocation_independent_canonical_identity() {
        let mut store = CanonicalTypeStore::new().unwrap();
        let never = store.never();
        let _ = store.primitive(PrimitiveType::I64).unwrap();

        assert_eq!(store.never(), never);
        assert!(matches!(store.kind(never), Ok(TypeKind::Never)));
        assert_eq!(PrimitiveType::from_spelling("Never"), None);
    }

    #[test]
    fn unions_flatten_deduplicate_and_reuse_identity() {
        let mut store = CanonicalTypeStore::new().unwrap();
        let left = store.primitive(PrimitiveType::I64).unwrap();
        let right = store.primitive(PrimitiveType::Str).unwrap();
        let inner = store.error_union([left, right]).unwrap();
        let outer = store.error_union([right, inner, left]).unwrap();

        assert_eq!(inner, outer);
        assert_eq!(store.error_union([left, left]).unwrap(), left);
        assert_eq!(store.error_union([]), Err(TypeStoreError::EmptyUnion));
    }

    #[test]
    fn unions_remove_never_members() {
        let mut store = CanonicalTypeStore::new().unwrap();
        let error = store.primitive(PrimitiveType::Str).unwrap();
        let never = store.never();

        assert_eq!(store.error_union([never, error]).unwrap(), error);
        assert_eq!(store.error_union([never, never]).unwrap(), never);
    }

    #[test]
    fn assignability_keeps_bottom_top_level_and_weakens_reference_access() {
        let mut store = CanonicalTypeStore::new().unwrap();
        let integer = store.primitive(PrimitiveType::I64).unwrap();
        let error = store.primitive(PrimitiveType::Str).unwrap();
        let never = store.never();
        let shared = store
            .reference(ReferenceMutability::Shared, integer)
            .unwrap();
        let mutable = store
            .reference(ReferenceMutability::Mutable, integer)
            .unwrap();
        let impossible_success = store
            .nominal(
                NominalHead::Intrinsic(IntrinsicNominal::Res),
                [never, error],
            )
            .unwrap();
        let integer_success = store
            .nominal(
                NominalHead::Intrinsic(IntrinsicNominal::Res),
                [integer, error],
            )
            .unwrap();

        assert!(store.is_assignable(never, integer).unwrap());
        assert!(store.is_assignable(mutable, shared).unwrap());
        assert!(!store.is_assignable(shared, mutable).unwrap());
        assert!(
            !store
                .is_assignable(impossible_success, integer_success)
                .unwrap()
        );
    }

    #[test]
    fn callable_assignability_is_contravariant_in_inputs_and_covariant_in_result() {
        let mut store = CanonicalTypeStore::new().unwrap();
        let integer = store.primitive(PrimitiveType::I64).unwrap();
        let text = store.primitive(PrimitiveType::Str).unwrap();
        let boolean = store.primitive(PrimitiveType::Bool).unwrap();
        let shared = store
            .reference(ReferenceMutability::Shared, integer)
            .unwrap();
        let mutable = store
            .reference(ReferenceMutability::Mutable, integer)
            .unwrap();
        let narrow_error = store
            .nominal(
                NominalHead::Intrinsic(IntrinsicNominal::Res),
                [integer, text],
            )
            .unwrap();
        let errors = store.error_union([text, boolean]).unwrap();
        let wide_error = store
            .nominal(
                NominalHead::Intrinsic(IntrinsicNominal::Res),
                [integer, errors],
            )
            .unwrap();
        let actual = store.callable([shared], narrow_error).unwrap();
        let expected = store.callable([mutable], wide_error).unwrap();
        let wrong_arity = store.callable([mutable, mutable], wide_error).unwrap();

        assert!(store.is_assignable(actual, expected).unwrap());
        assert!(!store.is_assignable(expected, actual).unwrap());
        assert!(!store.is_assignable(actual, wrong_arity).unwrap());
        assert!(!store.is_assignable(wrong_arity, actual).unwrap());
    }

    #[test]
    fn callable_bottom_compatibility_does_not_reverse_or_lift_through_results() {
        let mut store = CanonicalTypeStore::new().unwrap();
        let integer = store.primitive(PrimitiveType::I64).unwrap();
        let never = store.never();
        let returns_never = store.callable([integer], never).unwrap();
        let returns_integer = store.callable([integer], integer).unwrap();
        let accepts_never = store.callable([never], integer).unwrap();

        assert!(store.is_assignable(returns_never, returns_integer).unwrap());
        assert!(!store.is_assignable(returns_integer, returns_never).unwrap());
        assert!(!store.is_assignable(accepts_never, returns_integer).unwrap());
    }

    #[test]
    fn substitution_rewrites_complete_projection_identity() {
        let mut store = CanonicalTypeStore::new().unwrap();
        let owner = ExternalDefId::new("std::tensor::map").unwrap();
        let parameter = GenericParameterId::Trusted { owner, index: 0 };
        let parameter_type = store.generic_parameter(parameter.clone()).unwrap();
        let trait_arg = store.intern_substitution([parameter_type]).unwrap();
        let projection = store
            .associated_projection(AssociatedProjection::new(
                parameter_type,
                TraitRef::new(external("std::tensor::Readable"), trait_arg),
                external("std::tensor::Readable::Elem"),
            ))
            .unwrap();
        let concrete = store.primitive(PrimitiveType::F32).unwrap();
        let substituted = store
            .substitute(projection, &BTreeMap::from([(parameter, concrete)]))
            .unwrap();

        let TypeKind::AssociatedProjection(substituted) = store.kind(substituted).unwrap() else {
            panic!("projection must remain symbolic")
        };
        assert_eq!(substituted.base(), concrete);
        assert_eq!(
            store
                .substitution(substituted.trait_ref().substitution())
                .unwrap(),
            [concrete]
        );
    }

    #[test]
    fn empty_substitution_preserves_every_canonical_kind_without_payload_reads() {
        let mut store = CanonicalTypeStore::new().unwrap();
        let integer = store.primitive(PrimitiveType::I64).unwrap();
        let text = store.primitive(PrimitiveType::Str).unwrap();
        let parameter = store
            .generic_parameter(GenericParameterId::Trusted {
                owner: ExternalDefId::new("std::generic").unwrap(),
                index: 0,
            })
            .unwrap();
        let self_type = store.self_type(external("std::Node")).unwrap();
        let owned = store
            .nominal(NominalHead::Intrinsic(IntrinsicNominal::Own), [parameter])
            .unwrap();
        let reference = store
            .reference(ReferenceMutability::Mutable, parameter)
            .unwrap();
        let pointer = store
            .raw_pointer(RawPointerMutability::Const, parameter)
            .unwrap();
        let tuple = store.tuple([owned, reference, pointer, integer]).unwrap();
        let callable = store.callable([tuple, parameter], self_type).unwrap();
        let union = store.error_union([integer, text, callable]).unwrap();
        let arguments = store.intern_substitution([parameter, tuple]).unwrap();
        let projection = store
            .associated_projection(AssociatedProjection::new(
                parameter,
                TraitRef::new(external("std::Linked"), arguments),
                external("std::Linked::Item"),
            ))
            .unwrap();
        let roots = [
            store.never(),
            integer,
            text,
            parameter,
            self_type,
            owned,
            reference,
            pointer,
            tuple,
            callable,
            union,
            projection,
        ];
        let original = roots
            .iter()
            .map(|&root| store.kind(root).unwrap().clone())
            .collect::<Vec<_>>();
        let type_count = store.type_count();
        let substitution_count = store.substitution_count();
        SUBSTITUTION_READS.set(0);

        for (&root, kind) in roots.iter().zip(original) {
            assert_eq!(store.substitute(root, &BTreeMap::new()), Ok(root));
            assert_eq!(store.kind(root).unwrap(), &kind);
        }
        assert_eq!(SUBSTITUTION_READS.get(), 0);
        assert_eq!(store.type_count(), type_count);
        assert_eq!(store.substitution_count(), substitution_count);
    }

    #[test]
    fn empty_substitution_does_not_rescan_shared_suffixes_or_deep_graphs() {
        for size in [32, 128, 512, 20_000] {
            let mut store = CanonicalTypeStore::new().unwrap();
            let scalar = store.primitive(PrimitiveType::I64).unwrap();
            let mut root = scalar;
            let mut roots = Vec::new();
            for _ in 0..size {
                root = store.tuple([root, scalar]).unwrap();
                roots.push(root);
            }
            let type_count = store.type_count();
            SUBSTITUTION_READS.set(0);

            for root in roots {
                assert_eq!(store.substitute(root, &BTreeMap::new()), Ok(root));
            }
            assert_eq!(SUBSTITUTION_READS.get(), 0, "size={size}");
            assert_eq!(store.type_count(), type_count);
        }
    }

    #[test]
    fn substitution_preserves_root_and_replacement_validation_order() {
        let mut store = CanonicalTypeStore::new().unwrap();
        let scalar = store.primitive(PrimitiveType::I64).unwrap();
        let parameter = GenericParameterId::Trusted {
            owner: ExternalDefId::new("std::generic").unwrap(),
            index: 0,
        };
        let parameter_type = store.generic_parameter(parameter.clone()).unwrap();
        let root = store.tuple([parameter_type, scalar]).unwrap();
        let unknown_root = TypeId::from_index(store.type_count() + 10).unwrap();
        let unknown_value = TypeId::from_index(store.type_count() + 11).unwrap();
        let invalid = BTreeMap::from([(parameter.clone(), unknown_value)]);
        SUBSTITUTION_READS.set(0);

        assert_eq!(
            store.substitute(unknown_root, &BTreeMap::new()),
            Err(TypeStoreError::UnknownType(unknown_root)),
        );
        assert_eq!(
            store.substitute(unknown_root, &invalid),
            Err(TypeStoreError::UnknownType(unknown_root)),
        );
        assert_eq!(
            store.substitute(root, &invalid),
            Err(TypeStoreError::UnknownType(unknown_value)),
        );
        assert_eq!(SUBSTITUTION_READS.get(), 0);

        let concrete = store
            .substitute(root, &BTreeMap::from([(parameter, scalar)]))
            .unwrap();
        assert_eq!(concrete, store.tuple([scalar, scalar]).unwrap());
        assert_eq!(SUBSTITUTION_READS.get(), 3);
    }

    #[test]
    fn ownership_roots_are_distinct_from_nested_ownership() {
        let mut store = CanonicalTypeStore::new().unwrap();
        let payload = store.primitive(PrimitiveType::Str).unwrap();
        let error = store.primitive(PrimitiveType::I64).unwrap();
        let owned = store
            .nominal(NominalHead::Intrinsic(IntrinsicNominal::Own), [payload])
            .unwrap();
        let result = store
            .nominal(
                NominalHead::Intrinsic(IntrinsicNominal::Res),
                [owned, error],
            )
            .unwrap();
        let tuple = store.tuple([payload, owned]).unwrap();

        assert_eq!(
            store.ownership_root(result).unwrap(),
            Some(OwnershipRoot::Result { payload, error })
        );
        assert_eq!(store.ownership_root(tuple).unwrap(), None);
        assert!(store.contains_ownership(tuple).unwrap());

        let opt = NominalHead::Intrinsic(IntrinsicNominal::Opt);
        let optional = store.nominal(opt.clone(), [owned]).unwrap();
        assert_eq!(
            store.ownership_root(optional).unwrap(),
            Some(OwnershipRoot::Optional { payload })
        );
        let nested = store.nominal(opt.clone(), [result]).unwrap();
        assert_eq!(store.ownership_root(nested).unwrap(), None);
        assert!(store.contains_ownership(nested).unwrap());

        let mut deep = payload;
        for _ in 0..4_096 {
            deep = store.nominal(opt.clone(), [deep]).unwrap();
        }
        assert_eq!(store.ownership_root(deep).unwrap(), None);
    }

    #[test]
    fn projection_identity_metadata_does_not_leak_into_slot_properties() {
        let mut store = CanonicalTypeStore::new().unwrap();
        let payload = store.primitive(PrimitiveType::Str).unwrap();
        let owned = store
            .nominal(NominalHead::Intrinsic(IntrinsicNominal::Own), [payload])
            .unwrap();
        let arguments = store.intern_substitution([owned]).unwrap();
        let expected = AssociatedProjection::new(
            owned,
            TraitRef::new(external("std::Trait"), arguments),
            external("std::Trait::Item"),
        );
        let projection = store.associated_projection(expected.clone()).unwrap();

        assert!(!store.contains_ownership(projection).unwrap());
        assert!(!store.contains_reference(projection).unwrap());
        assert_eq!(
            store
                .payload_dependencies(projection)
                .unwrap()
                .collect::<Vec<_>>(),
            [PayloadDependency::Projection(&expected)]
        );
    }

    #[test]
    fn immediate_payload_dependencies_preserve_each_shape_and_child_order() {
        use PayloadDependency::Type;

        let mut store = CanonicalTypeStore::new().unwrap();
        let integer = store.primitive(PrimitiveType::I64).unwrap();
        let text = store.primitive(PrimitiveType::Str).unwrap();
        let parameter = store
            .generic_parameter(GenericParameterId::Trusted {
                owner: ExternalDefId::new("std::generic").unwrap(),
                index: 0,
            })
            .unwrap();
        let self_type = store.self_type(external("std::Node")).unwrap();
        let nominal = store
            .nominal(
                NominalHead::Intrinsic(IntrinsicNominal::Res),
                [text, integer],
            )
            .unwrap();
        let reference = store.reference(ReferenceMutability::Mutable, text).unwrap();
        let pointer = store
            .raw_pointer(RawPointerMutability::Const, integer)
            .unwrap();
        let tuple = store.tuple([text, integer, text]).unwrap();
        let callable = store.callable([text, integer, text], parameter).unwrap();
        let union = store.error_union([parameter, integer, parameter]).unwrap();
        for (root, expected) in [
            (store.never(), vec![]),
            (integer, vec![]),
            (parameter, vec![]),
            (self_type, vec![]),
            (nominal, vec![Type(text), Type(integer)]),
            (reference, vec![Type(text)]),
            (pointer, vec![Type(integer)]),
            (tuple, vec![Type(text), Type(integer), Type(text)]),
            (
                callable,
                vec![Type(text), Type(integer), Type(text), Type(parameter)],
            ),
            (union, vec![Type(integer), Type(parameter)]),
        ] {
            assert_eq!(
                store
                    .payload_dependencies(root)
                    .unwrap()
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn payload_queries_preserve_lookup_errors_and_predicate_short_circuiting() {
        let mut store = CanonicalTypeStore::new().unwrap();
        let unknown = TypeId::from_index(store.type_count() + 10).unwrap();
        assert_eq!(
            store
                .payload_dependencies(unknown)
                .map(|edges| edges.collect::<Vec<_>>()),
            Err(TypeStoreError::UnknownType(unknown)),
        );
        assert_eq!(
            store.tuple([unknown]),
            Err(TypeStoreError::UnknownType(unknown))
        );
        assert_eq!(
            store.reference(ReferenceMutability::Shared, unknown),
            Err(TypeStoreError::UnknownType(unknown)),
        );

        let invalid = SubstId::from_index(store.substitution_count() + 10).unwrap();
        let malformed = store
            .intern_type(TypeKind::Nominal {
                head: NominalHead::Intrinsic(IntrinsicNominal::Own),
                substitution: invalid,
            })
            .unwrap();
        assert!(store.contains_ownership(malformed).unwrap());
        assert_eq!(
            store.contains_reference(malformed),
            Err(TypeStoreError::UnknownSubstitution(invalid))
        );
        assert_eq!(
            store
                .payload_dependencies(malformed)
                .map(|edges| edges.collect::<Vec<_>>()),
            Err(TypeStoreError::UnknownSubstitution(invalid)),
        );
    }
}
