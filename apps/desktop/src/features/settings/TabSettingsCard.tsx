import cursorIconUrl from "../../shared/assets/icons/cursor.svg";
import type { TabMode, TabSettings } from "../../shared/api";
import { Button } from "../../shared/ui/Button";
import { Icon } from "../../shared/ui/Icon";
import { ModelSelect, type ModelSelectOption } from "../../shared/ui/ModelSelect";
import { Select } from "../../shared/ui/Select";
import { TitledCard } from "../../shared/ui/TitledCard";
import styles from "./TabSettingsCard.module.scss";

export function TabSettingsCard({
  settings,
  draft,
  modelOptions,
  editing,
  saving,
  onDraftChange,
  onEdit,
  onCancel,
  onSave,
}: {
  settings: TabSettings | null;
  draft: TabSettings;
  modelOptions: ModelSelectOption[];
  editing: boolean;
  saving: boolean;
  onDraftChange: (settings: TabSettings) => void;
  onEdit: () => void;
  onCancel: () => void;
  onSave: () => void;
}) {
  const modeLabel = (mode: TabMode) => {
    if (mode === "local") return t("内置 BYOK TAB");
    return t("Cursor 官方直连");
  };
  const activeSettings = editing ? draft : settings;
  const selectedModelLabel = settings?.model_id
    ? modelOptions.find((option) => option.value === settings.model_id)?.label
      ?? t("模型不可用：{model}", { model: settings.model_id })
    : t("停用（未选择模型）");
  const action = editing ? (
    <div className={styles.actionGroup}>
      <Button size="small" disabled={saving} onClick={onCancel}>{t("取消")}</Button>
      <Button variant="primary" size="small" disabled={saving} onClick={onSave}>{saving ? t("保存中…") : t("保存")}</Button>
    </div>
  ) : (
    <button type="button" className={styles.headerAction} disabled={!settings} onClick={onEdit}>{t("编辑")}</button>
  );

  return <TitledCard
    title={<div className={styles.title}><Icon src={cursorIconUrl} size="1.1em" /><span>{t("TAB 设置")}</span></div>}
    action={action}
  >
    <div className={styles.content}>
      <div className={styles.row}>
        <div className={styles.description}>
          <strong>{t("TAB 选择")}</strong>
          {editing && <small>{t("控制 Cursor TAB 相关接口的连接方式。")}</small>}
        </div>
        {editing ? <div className={styles.control}><Select
          value={draft.mode}
          disabled={saving}
          ariaLabel={t("TAB 选择")}
          options={[
            { value: "direct", label: t("Cursor 官方直连") },
            { value: "local", label: t("内置 BYOK TAB") },
          ]}
          onChange={(mode) => onDraftChange({ ...draft, mode: mode as TabMode })}
        /></div> : <span className={styles.value}>{settings ? modeLabel(settings.mode) : t("加载中…")}</span>}
      </div>
      {activeSettings?.mode === "local" && <>
        <div className={styles.row}>
          <strong>{t("TAB 补全模型")}</strong>
          {editing ? <div className={styles.control}><ModelSelect
            mode="single"
            value={draft.model_id}
            options={modelOptions}
            disabled={saving}
            label={t("TAB 补全模型")}
            onChange={(model_id) => onDraftChange({ ...draft, model_id })}
          /></div> : <span className={styles.value}>{selectedModelLabel}</span>}
        </div>
        <div className={styles.description}>
          <small>{t("选择低延迟模型；复用已有模型配置，无需填写新的 API Key，调用按所选模型服务计费。")}</small>
          <small>{t("无需 Cursor 官方登录。当前版本仅补全光标当前位置，不提供 Cursor 专有的多位置预测。")}</small>
          <small>{t("未选择模型时停用本地 TAB，不会自动使用 Agent 模型。")}</small>
        </div>
      </>}
      {activeSettings?.mode === "direct" && <div className={styles.description}>
        <small>{t("直连 Cursor 官方 TAB，需有效的 Cursor 登录账号和可用额度；未登录时不保证可用。")}</small>
      </div>}
    </div>
  </TitledCard>;
}
