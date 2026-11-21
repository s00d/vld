//! Macro bodies shared by the majors of one dependency. `$m` names the major's
//! module (`sea1`, `sea2`); each module exports a thin wrapper
//! (`__sea2_impl_vld_before_save` re-exported as `sea2::impl_vld_before_save`)
//! that forwards here with its own name.

#[cfg(any(feature = "sea-orm-1", feature = "sea-orm-2"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __sea_impl_vld_before_save {
    ($m:ident; $active_model:ty, $schema:ty) => {
        $crate::__sea_impl_vld_before_save!($m; $active_model, insert: $schema, update: $schema);
    };
    ($m:ident; $active_model:ty, insert: $ins_schema:ty, update: $upd_schema:ty) => {
        #[$crate::$m::sea_orm::prelude::async_trait::async_trait]
        impl $crate::$m::sea_orm::ActiveModelBehavior for $active_model {
            async fn before_save<C: $crate::$m::sea_orm::ConnectionTrait>(
                self,
                _db: &C,
                insert: bool,
            ) -> Result<Self, $crate::$m::sea_orm::DbErr> {
                if insert {
                    $crate::$m::before_save::<$ins_schema, _>(&self)?;
                } else {
                    $crate::$m::before_save::<$upd_schema, _>(&self)?;
                }
                Ok(self)
            }
        }
    };
}
