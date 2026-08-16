export interface ProviderModel {
  id: string;
  label: string;
}

export interface CatalogEntry {
  id: string;
  models: ProviderModel[];
}
