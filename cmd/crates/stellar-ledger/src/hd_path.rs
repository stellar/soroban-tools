use crate::Error;

#[derive(Clone, Copy)]
pub struct HdPath(pub u32);

impl HdPath {
    #[must_use]
    pub fn depth(&self) -> u8 {
        let path: slip10::BIP32Path = self.into();
        path.depth()
    }
}

impl From<u32> for HdPath {
    fn from(index: u32) -> Self {
        HdPath(index)
    }
}

impl From<&u32> for HdPath {
    fn from(index: &u32) -> Self {
        HdPath(*index)
    }
}

impl HdPath {
    /// # Errors
    ///
    /// Could fail to convert the path to bytes
    pub fn to_vec(&self) -> Result<Vec<u8>, Error> {
        hd_path_to_bytes(&self.into())
    }
}

impl From<&HdPath> for slip10::BIP32Path {
    fn from(value: &HdPath) -> Self {
        let index = value.0;
        format!("m/44'/148'/{index}'").parse().unwrap()
    }
}

fn hd_path_to_bytes(hd_path: &slip10::BIP32Path) -> Result<Vec<u8>, Error> {
    let hd_path_indices = 0..hd_path.depth();
    let result = hd_path_indices
        .into_iter()
        .map(|index| {
            Ok(hd_path
                .index(index)
                .ok_or_else(|| Error::Bip32PathError(format!("{hd_path}")))?
                .to_be_bytes())
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(result.into_iter().flatten().collect())
}
