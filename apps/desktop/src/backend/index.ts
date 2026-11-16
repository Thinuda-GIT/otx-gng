/** Backend selection.
 *  * Browser preview (`npm run dev`): MockBackend, everything fake.
 *  * Desktop app (Tauri): TauriBackend, real system facts and Core status; other features are
 *    still mocked until their phase lands (the "Preview data" chip stays visible). */
import { isTauri } from "../lib/tauri";
import { MockBackend } from "./mock";
import { TauriBackend } from "./tauri";
import type { Backend } from "./types";

export const mockBackend: MockBackend = isTauri() ? new TauriBackend() : new MockBackend();
export const backend: Backend = mockBackend;
/** True while any page still shows mock data (until the final phase). */
export const isPreviewData = true;
export const hasRealSystemInfo = backend.kind === "tauri";

export type * from "./types";
