use crate::material::Package;

pub(super) fn seal(package: Package) -> Result<Package, String> {
    package.sealed()
}
