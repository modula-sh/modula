import type { CatalogEntry } from "../../../lib/providerCatalog";
import { call } from "../invoke";

export class CatalogResource {
  providerCatalog() {
    return call<CatalogEntry[]>("provider_catalog");
  }
}
