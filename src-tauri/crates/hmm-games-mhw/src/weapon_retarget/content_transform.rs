use super::{
    analyze_mhw_weapon_assets, transform_mhw_weapon_mrl3_texture_paths, WeaponBinaryError,
    WeaponMainId, WeaponModelAssetKind, WeaponModelPair,
    MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID, MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_VERSION,
};
use hmm_core::ContentTransformInvocation;
use hmm_ports::{
    ContentTransformOutput, ContentTransformRequest, ContentTransformer, ContentTransformerError,
    ReplacementAsset,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const SOURCE_RELATIVE_PATH_PARAMETER: &str = "source_relative_path";
const COMPANION_RELATIVE_PATH_PARAMETER: &str = "companion_relative_path";
const TARGET_MAIN_ID_PARAMETER: &str = "target_main_id";

pub struct MhwWeaponMrl3TexturePathTransformer;

impl ContentTransformer for MhwWeaponMrl3TexturePathTransformer {
    fn transformer_id(&self) -> &'static str {
        MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID
    }

    fn transformer_version(&self) -> u32 {
        MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_VERSION
    }

    fn transform(
        &self,
        request: ContentTransformRequest<'_>,
    ) -> Result<ContentTransformOutput, ContentTransformerError> {
        let parameters = request.invocation().parameters();
        if parameters.len() != 3 || request.dependencies().len() != 1 {
            return Err(ContentTransformerError::InvalidInvocation);
        }
        let source_relative_path = parameters
            .get(SOURCE_RELATIVE_PATH_PARAMETER)
            .ok_or(ContentTransformerError::InvalidInvocation)?;
        let companion_relative_path = parameters
            .get(COMPANION_RELATIVE_PATH_PARAMETER)
            .ok_or(ContentTransformerError::InvalidInvocation)?;
        let target_main_id = parameters
            .get(TARGET_MAIN_ID_PARAMETER)
            .ok_or(ContentTransformerError::InvalidInvocation)?;
        let (companion_package_file_id, companion_bytes) = request
            .dependencies()
            .first_key_value()
            .ok_or(ContentTransformerError::DependencyUnavailable)?;

        let assets = [
            ReplacementAsset::new(request.package_file_id().clone(), source_relative_path),
            ReplacementAsset::new(companion_package_file_id.clone(), companion_relative_path),
        ];
        let analysis = analyze_mhw_weapon_assets(&assets)
            .map_err(|error| ContentTransformerError::rejected(error.code()))?;
        // 这里的输入恰好是一对 MOD3/MRL3，因此必然只有一个单元、一个模型对。
        // 多于一个就说明调用方传错了资源，属于 `InvalidInvocation` 而不是包的问题。
        let [closure] = analysis.units() else {
            return Err(ContentTransformerError::InvalidInvocation);
        };
        let pair = closure
            .pairs()
            .first()
            .filter(|_| closure.pairs().len() == 1)
            .ok_or(ContentTransformerError::InvalidInvocation)?;
        if pair.mrl3().package_file_id() != request.package_file_id()
            || pair.mrl3().kind() != WeaponModelAssetKind::Mrl3
            || pair.mod3().package_file_id() != companion_package_file_id
            || pair.mod3().kind() != WeaponModelAssetKind::Mod3
        {
            return Err(ContentTransformerError::InvalidInvocation);
        }
        let target_main_id = WeaponMainId::parse_for_family(target_main_id, closure.family())
            .map_err(|_| ContentTransformerError::rejected("weapon_invalid_main_id"))?;
        let transformed = transform_mhw_weapon_mrl3_texture_paths(
            pair,
            &target_main_id,
            companion_bytes,
            request.source_bytes(),
        )
        .map_err(|error| ContentTransformerError::rejected(error.code()))?;
        let mapping_sha256 = transformed.report().mapping_sha256().to_owned();
        Ok(ContentTransformOutput::new(
            transformed.into_bytes(),
            mapping_sha256,
        ))
    }
}

pub fn build_mhw_weapon_mrl3_transform_invocation(
    pair: &WeaponModelPair,
    target_main_id: &WeaponMainId,
    mod3_bytes: &[u8],
    mrl3_bytes: &[u8],
) -> Result<ContentTransformInvocation, WeaponBinaryError> {
    let transformed =
        transform_mhw_weapon_mrl3_texture_paths(pair, target_main_id, mod3_bytes, mrl3_bytes)?;
    let report = transformed.report();
    ContentTransformInvocation::new(
        1,
        report.transformer_id(),
        report.transformer_version(),
        report.source_sha256(),
        report.output_sha256(),
        report.mapping_sha256(),
        BTreeMap::from([(
            pair.mod3().package_file_id().clone(),
            sha256_hex(mod3_bytes),
        )]),
        BTreeMap::from([
            (
                SOURCE_RELATIVE_PATH_PARAMETER.to_owned(),
                pair.mrl3().relative_path().as_str().to_owned(),
            ),
            (
                COMPANION_RELATIVE_PATH_PARAMETER.to_owned(),
                pair.mod3().relative_path().as_str().to_owned(),
            ),
            (
                TARGET_MAIN_ID_PARAMETER.to_owned(),
                target_main_id.as_str().to_owned(),
            ),
        ]),
    )
    .map_err(|_| WeaponBinaryError::OutputInvalid)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
