import { useState } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { retargetFileCopy } from "./retargetFileCopy";
import type { RetargetFilePreview } from "./retargetFileTypes";
import "./RetargetFileDetails.css";

export function RetargetFileDetails({ files, sourceLabels = {} }: {
  files?: readonly RetargetFilePreview[];
  sourceLabels?: Readonly<Record<string, string>>;
}) {
  const { locale } = useI18n();
  const copy = resolveCopy(retargetFileCopy, locale);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [limit, setLimit] = useState(100);
  if (!files?.length) return null;
  const needle = query.trim().toLocaleLowerCase();
  const matches = needle ? files.filter((file) => [file.sourcePath, file.installedPath, file.targetPath,
    file.sourceId ? sourceLabels[file.sourceId] : null].filter(Boolean).join(" ").toLocaleLowerCase().includes(needle)) : files;
  const visible = matches.slice(0, limit);
  const counts = files.reduce<Record<string, number>>((result, file) => {
    result[file.disposition] = (result[file.disposition] ?? 0) + 1;
    return result;
  }, {});
  return <details className="retarget-files" onToggle={(event) => setOpen(event.currentTarget.open)}>
    <summary>{copy.title(files.length)}</summary>
    {open && <div className="retarget-files__body">
      <div className="retarget-files__counts">{Object.entries(counts).map(([kind, count]) => <span key={kind}>
        {copy.dispositions[kind as keyof typeof copy.dispositions] ?? copy.unknown} <strong>{count}</strong>
      </span>)}</div>
      <label className="retarget-files__search">{copy.search}<input type="search" value={query} onChange={(event) => { setQuery(event.target.value); setLimit(100); }} /></label>
      <p className="retarget-files__shown" role="status">{copy.shown(visible.length, matches.length)}</p>
      {matches.length === 0 && <p>{copy.empty}</p>}
      <ul className="retarget-files__list">{visible.map((file) => <li key={file.fileId}>
        <div className="retarget-files__heading">
          <strong>{file.sourceId ? sourceLabels[file.sourceId] ?? copy.equipment : copy.package}</strong>
          <span>{copy.dispositions[file.disposition] ?? copy.unknown}{file.change ? ` · ${copy.changes[file.change]}` : ""}</span>
        </div>
        <p>{copy.reasons[file.reason] ?? copy.unknown}</p>
        <dl>
          <div><dt>{copy.source}</dt><dd><code>{file.sourcePath}</code></dd></div>
          <div><dt>{copy.installed}</dt><dd>{file.installedPath ? <code>{file.installedPath}</code> : copy.notManaged}</dd></div>
          <div><dt>{copy.target}</dt><dd>{file.targetPath ? <code>{file.targetPath}</code> : copy.excluded}</dd></div>
        </dl>
      </li>)}</ul>
      {visible.length < matches.length && <button type="button" onClick={() => setLimit((value) => value + 100)}>{copy.more}</button>}
    </div>}
  </details>;
}
