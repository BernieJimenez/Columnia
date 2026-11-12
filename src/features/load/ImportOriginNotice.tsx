import type { ImportOrigin } from "../../bridge";
import { importOriginNotice } from "./importOrigin";

/**
 * AUD-061: a line under the dataset name in the top bar when the dataset is a
 * part of a database table or had texts cut or replaced, so it is not only
 * said in Cargar. It is plain text: it does not take over the save status.
 */
export function ImportOriginNotice({ origin }: { origin: ImportOrigin | null }) {
  const notice = importOriginNotice(origin);
  if (!notice) return null;
  return (
    <p className="dataset-bar__origin" title={notice.title}>
      {notice.text}
    </p>
  );
}
