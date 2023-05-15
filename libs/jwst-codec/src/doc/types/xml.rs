use crate::wrap_inner;

pub struct YXMLElementInner {}

pub struct YXMLFragmentInner {}

pub struct YXMLTextInner {}

wrap_inner!(
    (YXMLElement, YXMLElementInner),
    (YXMLFragment, YXMLFragmentInner),
    (YXMLText, YXMLTextInner)
);
