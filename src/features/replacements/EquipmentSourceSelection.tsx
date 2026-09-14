import { useMemo, useState, type ReactNode } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import type { InstallManifestStatus } from "../mods/modInstallPlanTypes";
import type { EquipmentRetargetConfiguration, EquipmentSourceConfiguration, EquipmentTargetChoice, EquipmentTargetChoices } from "./equipmentRetargetTypes";
import { equipmentRetargetCopy } from "./equipmentRetargetCopy";
import { replacementCopy } from "./replacementCopy";
import { replacementIdentityLabel, replacementKindLabel } from "./replacementIdentityLabel";
import { buildReplacementTargetOptions, replacementTargetOption } from "./replacementTargetOptions";
import { ReplacementTargetCatalog } from "./ReplacementTargetCatalog";
import { retargetDialogCopy } from "./retargetDialogCopy";
import { RetargetPopover } from "./RetargetPopover";

export function EquipmentSourceSelection({ configuration, choices, installStatus, disabled, onChoose, tools }: {
  configuration: EquipmentRetargetConfiguration; choices: EquipmentTargetChoices; installStatus: InstallManifestStatus | undefined; disabled: boolean;
  onChoose: (sourceId: string, choice: EquipmentTargetChoice) => void; tools: ReactNode;
}) {
  const { locale } = useI18n();
  const copy = resolveCopy(retargetDialogCopy, locale);
  const groupCopy = resolveCopy(equipmentRetargetCopy, locale);
  const [activeSource, setActiveSource] = useState(configuration.sources[0]?.source.id);
  const active = configuration.sources.some(({ source }) => source.id === activeSource) ? activeSource : configuration.sources[0]?.source.id;

  return <div className="equipment-retarget">
    <div className="equipment-retarget__toolbar"><span title={groupCopy.hint}>{copy.sources(configuration.sources.length)}</span>{tools}</div>
    <nav className="equipment-retarget__sources" aria-label={groupCopy.title}>
      {configuration.sources.map((item) => {
        const choice = choices[item.source.id];
        const target = item.targets.find((candidate) => candidate.id === choice?.targetId);
        const option = target ? replacementTargetOption(target, locale, choice?.alias ?? null) : null;
        const selected = target && option ? replacementIdentityLabel(target, locale, option.displayName)
          : choice ? copy.unavailableTarget : replacementIdentityLabel(item.source, locale);
        return <button type="button" key={item.source.id} aria-pressed={active === item.source.id} data-source-id={item.source.id}
          onClick={() => setActiveSource(item.source.id)}>
          <strong>{replacementKindLabel(item.source.sourceType, locale)} · {replacementIdentityLabel(item.source, locale)}</strong>
          <span>{copy.selected}：{selected}</span>
        </button>;
      })}
    </nav>
    {configuration.sources.map((item) => <section key={item.source.id} className="equipment-retarget__source" hidden={active !== item.source.id} data-source-id={item.source.id}>
      <EquipmentSourcePicker item={item} choice={choices[item.source.id] ?? null} installedTargetId={configuration.installedTargets?.[item.source.id]}
        installStatus={installStatus} disabled={disabled} onChoose={(choice) => onChoose(item.source.id, choice)} />
    </section>)}
  </div>;
}

function EquipmentSourcePicker({ item, choice, installedTargetId, installStatus, disabled, onChoose }: {
  item: EquipmentSourceConfiguration; choice: EquipmentTargetChoice; installedTargetId: string | undefined;
  installStatus: InstallManifestStatus | undefined; disabled: boolean; onChoose: (choice: EquipmentTargetChoice) => void;
}) {
  const { locale } = useI18n();
  const copy = resolveCopy(replacementCopy, locale);
  const groupCopy = resolveCopy(equipmentRetargetCopy, locale);
  const dialogCopy = resolveCopy(retargetDialogCopy, locale);
  const installed = installStatus === "installed";
  const [query, setQuery] = useState("");
  const selectedTarget = item.targets.find((target) => target.id === choice?.targetId);
  const selectedOption = selectedTarget ? replacementTargetOption(selectedTarget, locale, choice?.alias ?? null) : null;
  const options = useMemo(() => buildReplacementTargetOptions(item.targets, locale, query), [item.targets, locale, query]);
  const visible = selectedOption && !options.some((option) => option.key === selectedOption.key) ? [selectedOption, ...options] : options;
  const currentTarget = item.targets.find((target) => target.id === installedTargetId);
  const sourceLabel = replacementIdentityLabel(item.source, locale);
  const isOriginal = installedTargetId !== undefined && installedTargetId === item.originalTargetId;
  const currentLabel = isOriginal ? sourceLabel : currentTarget ? replacementIdentityLabel(currentTarget, locale) : copy.panel.currentTargetsUnknown;
  const shared = selectedTarget ? buildReplacementTargetOptions([selectedTarget], locale) : [];

  return <>
    <div className="equipment-retarget__context">
      {installed && isOriginal ? <span>{dialogCopy.currentAndDefault}：<strong>{sourceLabel}</strong></span> : <>
        <span>{dialogCopy.original}：<strong>{sourceLabel}</strong></span>
        {installed ? <span>{dialogCopy.current}：<strong>{currentLabel}</strong></span>
          : <span>{installStatus === "not_installed" ? dialogCopy.notInstalled : copy.panel.currentTargetsUnknown}</span>}
      </>}
      <button type="button" className="equipment-retarget__keep" aria-pressed={choice === null}
        disabled={disabled || item.originalTargetId === null} onClick={() => onChoose(null)}>{groupCopy.keep}</button>
    </div>
    <ReplacementTargetCatalog options={visible} query={query} onQueryChange={setQuery} selectedOption={selectedOption} disabled={disabled}
      installedTargetId={installedTargetId} onSelect={(targetId, alias) => onChoose({ targetId, alias })} />
    {shared.length > 1 && <RetargetPopover title={copy.panel.selectedAliasesTitle} trigger={copy.panel.selectedAliasesCount(shared.length)}>
      <p>{copy.panel.selectedAliasesHint}</p>
      <ul>{shared.map((option) => <li key={option.key}>{replacementIdentityLabel(option.target, locale, option.displayName)}</li>)}</ul>
    </RetargetPopover>}
  </>;
}
