use serde::{Deserialize, Serialize};
use xodus::{models::secrets::Token, tokens::TokenManager};

#[derive(Serialize, Deserialize)]
pub struct Device {
    sts: xodus::models::secrets::LegacyToken,
    license: xodus::models::secrets::Device,
}

impl Device {
    pub fn fill(&self, mgr: &TokenManager) -> Result<(), xodus::tokens::store::TokenStoreError> {
        mgr.save_device_license(&self.license)?;
        mgr.save_device_token(self.sts.key_name.as_ref().cloned().unwrap(), Token::Legacy(self.sts.clone()))
    }
    pub fn from(mgr: &TokenManager) -> Result<Device, xodus::tokens::store::TokenStoreError> {
        let u = mgr.get_device_license()?;
        let t = mgr.get_device_sts_token()?;
        let Token::Legacy(lt) = t else {
            return Err(xodus::tokens::store::TokenStoreError::NotFound)
        };
        Ok(Device { license: u, sts: lt })
    }
}

#[derive(Serialize, Deserialize)]
pub struct User {
    puid: String,
    username: String,
    sts: xodus::models::secrets::LegacyToken,
}

impl User {
    pub fn fill(&self, mgr: &TokenManager) -> Result<(), xodus::tokens::store::TokenStoreError> {
        mgr.save_user(&xodus::models::secrets::User{
            puid: self.puid.clone(),
            username: self.username.clone(),
        })?;
        mgr.save_user_token(self.sts.key_name.as_ref().cloned().unwrap_or("http://Passport.NET/STS".to_owned()), Token::Legacy(self.sts.clone()))
    }

    pub fn from(mgr: &TokenManager) -> Result<User, xodus::tokens::store::TokenStoreError> {
        let u = mgr.get_user()?;
        let t = mgr.get_user_sts_token()?;
        let Token::Legacy(lt) = t else {
            return Err(xodus::tokens::store::TokenStoreError::NotFound)
        };
        Ok(User { puid: u.puid, username: u.username, sts: lt })
    }
}
