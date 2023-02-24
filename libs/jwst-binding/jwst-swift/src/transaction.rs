use yrs::TransactionMut;

pub struct Transaction<'a> {
    pub(crate) trx: TransactionMut<'a>,
}
