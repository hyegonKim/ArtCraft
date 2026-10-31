-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

update model_weights
set
    mod_deleted_at = NOW()
where token IN (
    'weight_tnhacmxb9qwqrygc3pxvk3ds6',
    'weight_9g1pghkc0xhfy5n9m36b11vds',
    'weight_5k0kzkftr4x39xyc9yg5p9c86'
);

