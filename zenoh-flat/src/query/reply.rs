use crate::{Encoding, Sample, ZBytes, ZReply, ZenohId};
use prebindgen_proc_macro::prebindgen;

/// Data-class twin of [`ZReply`]. A success carries `sample`; an error
/// carries `error_payload` + `error_encoding`. `replier_*` identify the
/// node that answered (zeroed/`None` when unknown).
#[prebindgen]
pub struct Reply {
    pub replier_zid: Option<ZenohId>,
    pub replier_eid: i32,
    pub sample: Option<Sample>,
    pub error_payload: Option<ZBytes>,
    pub error_encoding: Option<Encoding>,
}

impl From<&ZReply> for Reply {
    fn from(r: &ZReply) -> Self {
        let (replier_zid, replier_eid) = r
            .replier_id()
            .map(|id| (Some(ZenohId::from(id.zid())), id.eid() as i32))
            .unwrap_or((None, 0));
        match r.result() {
            Ok(sample) => Reply {
                replier_zid,
                replier_eid,
                sample: Some(Sample::from(sample)),
                error_payload: None,
                error_encoding: None,
            },
            Err(err) => Reply {
                replier_zid,
                replier_eid,
                sample: None,
                error_payload: Some(ZBytes::from(err.payload().clone())),
                error_encoding: Some(Encoding::from(err.encoding())),
            },
        }
    }
}
