use crate::span::Span;

#[derive(Debug, Clone)]
pub struct Document {
    pub decls: Vec<Decl>,
}

#[derive(Debug, Clone)]
pub struct Decl {
    pub kind: DeclKind,
    pub id: String,
    pub form: DeclForm,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeclKind {
    Equipment,
    Valve,
    Line,
    Instrument,
    Signal,
    Group,
    Area,
    Note,
    Junction,
}

impl DeclKind {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "equipment" => Some(DeclKind::Equipment),
            "valve" => Some(DeclKind::Valve),
            "line" => Some(DeclKind::Line),
            "instrument" => Some(DeclKind::Instrument),
            "signal" => Some(DeclKind::Signal),
            "group" => Some(DeclKind::Group),
            "area" => Some(DeclKind::Area),
            "note" => Some(DeclKind::Note),
            "junction" => Some(DeclKind::Junction),
            _ => None,
        }
    }
}

impl std::fmt::Display for DeclKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DeclKind::Equipment => "equipment",
            DeclKind::Valve => "valve",
            DeclKind::Line => "line",
            DeclKind::Instrument => "instrument",
            DeclKind::Signal => "signal",
            DeclKind::Group => "group",
            DeclKind::Area => "area",
            DeclKind::Note => "note",
            DeclKind::Junction => "junction",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone)]
pub enum DeclForm {
    Inline(Vec<Prop>),
    Block(Vec<Prop>),
}

impl DeclForm {
    pub fn props(&self) -> &[Prop] {
        match self {
            DeclForm::Inline(props) | DeclForm::Block(props) => props,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Prop {
    pub key: String,
    pub value: PropVal,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum PropVal {
    Value(Value),
    Block(Vec<Prop>),
}

#[derive(Debug, Clone)]
pub enum Value {
    Str(String),
    Ref(String, Option<String>), // object_id, optional port
    Tuple(Vec<i32>),
    List(Vec<String>),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Str(s) => write!(f, "{}", s),
            Value::Ref(id, None) => write!(f, "{}", id),
            Value::Ref(id, Some(port)) => write!(f, "{}.{}", id, port),
            Value::Tuple(nums) => {
                write!(f, "(")?;
                for (i, n) in nums.iter().enumerate() {
                    if i > 0 { write!(f, ",")?; }
                    write!(f, "{}", n)?;
                }
                write!(f, ")")
            }
            Value::List(items) => {
                for (i, s) in items.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", s)?;
                }
                Ok(())
            }
        }
    }
}
