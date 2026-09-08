import { useId, type KeyboardEvent } from "react";
import { coreLocales, localeMeta, resolveCopy, useI18n } from "../../shared/i18n";
import { onboardingTourCopy } from "./onboardingTourCopy";

export function TourLanguagePicker() {
  const { locale, preference, systemLocale, setPreference } = useI18n();
  const copy = resolveCopy(onboardingTourCopy, locale).language;
  const groupId = useId();

  function keepRadioNavigation(event: KeyboardEvent<HTMLFieldSetElement>) {
    // 方向键交给原生 radio group，不能冒泡成引导的上一项/下一项。
    if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) {
      event.stopPropagation();
    }
  }

  return (
    <fieldset className="tour-language-picker" aria-label={copy.title} onKeyDown={keepRadioNavigation}>
      {coreLocales.map((option) => (
        <label key={option} className="tour-language-picker__option">
          <input
            type="radio"
            name={groupId}
            value={option}
            checked={preference === option}
            onChange={() => setPreference(option)}
          />
          <span lang={localeMeta[option].bcp47}>{localeMeta[option].nativeName}</span>
        </label>
      ))}
      <label className="tour-language-picker__system">
        <input
          type="radio"
          name={groupId}
          value="system"
          checked={preference === "system"}
          onChange={() => setPreference("system")}
        />
        <span>{copy.system} ({localeMeta[systemLocale].nativeName})</span>
      </label>
    </fieldset>
  );
}
