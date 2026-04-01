import { useEffect } from 'react';
import { ReloadOutlined } from '@ant-design/icons';
import {
  Alert,
  Button,
  Card,
  Checkbox,
  Col,
  Descriptions,
  Form,
  Input,
  List,
  Row,
  Space,
  Spin,
  Tag,
  Typography,
  message,
} from 'antd';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { desktopApi } from '../services/desktopApi';
import type {
  DesktopAccountStatus,
  DesktopConnectionDiagnostics,
  DesktopPairingRequest,
  DesktopSettings,
  DesktopWorkspaceStatus,
} from '../types/desktop';

const Settings: React.FC = () => {
  const [form] = Form.useForm<DesktopSettings>();
  const queryClient = useQueryClient();

  const settingsQuery = useQuery({
    queryKey: ['desktop-settings'],
    queryFn: desktopApi.getSettings,
  });

  const defaultSettingsQuery = useQuery({
    queryKey: ['desktop-default-settings'],
    queryFn: desktopApi.getDefaultSettings,
  });

  const capabilityStatusQuery = useQuery({
    queryKey: ['desktop-capability-status'],
    queryFn: desktopApi.getCapabilityStatus,
  });

  const workspaceStatusQuery = useQuery({
    queryKey: ['desktop-workspace-status'],
    queryFn: desktopApi.getWorkspaceStatus,
  });

  const accountStatusQuery = useQuery({
    queryKey: ['desktop-account-status'],
    queryFn: desktopApi.getAccountStatus,
    refetchInterval: 5000,
  });

  const connectionDiagnosticsQuery = useQuery({
    queryKey: ['desktop-connection-diagnostics'],
    queryFn: desktopApi.getConnectionDiagnostics,
  });

  const validateMutation = useMutation({
    mutationFn: desktopApi.validateWorkspace,
  });

  const pickWorkspaceMutation = useMutation({
    mutationFn: desktopApi.pickWorkspace,
    onSuccess: ({ path }) => {
      form.setFieldValue('workspace_path', path);
      message.success('已选择工作区');
    },
  });

  const notificationMutation = useMutation({
    mutationFn: desktopApi.testNotification,
    onSuccess: (text) => {
      message.success(text);
      void queryClient.invalidateQueries({ queryKey: ['desktop-capability-status'] });
    },
  });

  const startPairingMutation = useMutation({
    mutationFn: desktopApi.startAccountPairing,
    onSuccess: async (pairing) => {
      message.success(`绑定码已生成：${pairing.pairing_code}`);
      await queryClient.invalidateQueries({ queryKey: ['desktop-account-status'] });
    },
  });

  const cancelPairingMutation = useMutation({
    mutationFn: desktopApi.cancelAccountPairing,
    onSuccess: async (text) => {
      message.success(text);
      await queryClient.invalidateQueries({ queryKey: ['desktop-account-status'] });
    },
  });

  const deleteBindingMutation = useMutation({
    mutationFn: desktopApi.deleteAccountBinding,
    onSuccess: async (text) => {
      message.success(text);
      await queryClient.invalidateQueries({ queryKey: ['desktop-account-status'] });
    },
  });

  const recoverConnectionMutation = useMutation({
    mutationFn: desktopApi.recoverConnection,
    onSuccess: async (result) => {
      if (result.success) {
        message.success(result.message);
      } else {
        message.warning(result.message);
      }
      queryClient.setQueryData(['desktop-connection-diagnostics'], result.status);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ['desktop-connection-diagnostics'] }),
        queryClient.invalidateQueries({ queryKey: ['desktop-runtime-status'] }),
        queryClient.invalidateQueries({ queryKey: ['desktop-account-status'] }),
        queryClient.invalidateQueries({ queryKey: ['desktop-workspace-status'] }),
        queryClient.invalidateQueries({ queryKey: ['desktop-logs'] }),
      ]);
    },
  });

  const saveMutation = useMutation({
    mutationFn: desktopApi.saveSettings,
    onSuccess: async (saved) => {
      queryClient.setQueryData(['desktop-settings'], saved);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ['desktop-workspace-status'] }),
        queryClient.invalidateQueries({ queryKey: ['desktop-runtime-status'] }),
        queryClient.invalidateQueries({ queryKey: ['desktop-version-summary'] }),
        queryClient.invalidateQueries({ queryKey: ['desktop-capability-status'] }),
        queryClient.invalidateQueries({ queryKey: ['desktop-account-status'] }),
      ]);

      const latestWorkspaceStatus = queryClient.getQueryData<DesktopWorkspaceStatus>(['desktop-workspace-status']);
      if (latestWorkspaceStatus?.restart_required) {
        message.warning(latestWorkspaceStatus.restart_notice || '设置已保存，重启 Node 后生效');
      } else {
        message.success('设置已保存');
      }
    },
  });

  useEffect(() => {
    if (settingsQuery.data) {
      form.setFieldsValue(settingsQuery.data);
    }
  }, [form, settingsQuery.data]);

  if (settingsQuery.isLoading && !settingsQuery.data) {
    return (
      <div style={{ textAlign: 'center', padding: 48 }}>
        <Spin size="large" />
      </div>
    );
  }

  if (settingsQuery.error instanceof Error && !settingsQuery.data) {
    return <Alert type="error" showIcon message="加载设置失败" description={settingsQuery.error.message} />;
  }

  const saving = saveMutation.isPending;
  const validating = validateMutation.isPending;
  const pickingWorkspace = pickWorkspaceMutation.isPending;
  const sendingNotification = notificationMutation.isPending;
  const pairingBusy =
    startPairingMutation.isPending || cancelPairingMutation.isPending || deleteBindingMutation.isPending;
  const workspaceStatus = workspaceStatusQuery.data;
  const accountStatus = accountStatusQuery.data;
  const connectionDiagnostics = connectionDiagnosticsQuery.data;
  const activePairing = accountStatus?.pairing || null;
  const validation = validateMutation.data;
  const defaults = defaultSettingsQuery.data;
  const capabilityStatus = capabilityStatusQuery.data;
  const accountStatusMissingToken =
    accountStatusQuery.error instanceof Error && accountStatusQuery.error.message.includes('Node auth token is missing');

  return (
    <Space direction="vertical" size={16} style={{ width: '100%' }}>
      {settingsQuery.error instanceof Error ? (
        <Alert type="error" showIcon message="刷新设置失败" description={settingsQuery.error.message} />
      ) : null}
      {defaultSettingsQuery.error instanceof Error ? (
        <Alert type="error" showIcon message="加载默认值失败" description={defaultSettingsQuery.error.message} />
      ) : null}
      {capabilityStatusQuery.error instanceof Error ? (
        <Alert type="error" showIcon message="加载桌面能力失败" description={capabilityStatusQuery.error.message} />
      ) : null}
      {workspaceStatusQuery.error instanceof Error ? (
        <Alert
          type="error"
          showIcon
          message="加载工作区状态失败"
          description={workspaceStatusQuery.error.message}
        />
      ) : null}
      {accountStatusQuery.error instanceof Error && !accountStatusMissingToken ? (
        <Alert
          type="error"
          showIcon
          message="加载账号绑定状态失败"
          description={accountStatusQuery.error.message}
        />
      ) : null}
      {connectionDiagnosticsQuery.error instanceof Error ? (
        <Alert
          type="error"
          showIcon
          message="加载连接诊断失败"
          description={connectionDiagnosticsQuery.error.message}
        />
      ) : null}
      {saveMutation.error instanceof Error ? (
        <Alert type="error" showIcon message="保存设置失败" description={saveMutation.error.message} />
      ) : null}
      {validateMutation.error instanceof Error ? (
        <Alert type="error" showIcon message="校验工作区失败" description={validateMutation.error.message} />
      ) : null}
      {pickWorkspaceMutation.error instanceof Error ? (
        <Alert type="error" showIcon message="选择工作区失败" description={pickWorkspaceMutation.error.message} />
      ) : null}
      {notificationMutation.error instanceof Error ? (
        <Alert type="error" showIcon message="发送测试通知失败" description={notificationMutation.error.message} />
      ) : null}
      {startPairingMutation.error instanceof Error ? (
        <Alert type="error" showIcon message="发起账号绑定失败" description={startPairingMutation.error.message} />
      ) : null}
      {cancelPairingMutation.error instanceof Error ? (
        <Alert type="error" showIcon message="取消账号绑定失败" description={cancelPairingMutation.error.message} />
      ) : null}
      {deleteBindingMutation.error instanceof Error ? (
        <Alert type="error" showIcon message="解绑账号失败" description={deleteBindingMutation.error.message} />
      ) : null}
      {recoverConnectionMutation.error instanceof Error ? (
        <Alert type="error" showIcon message="执行连接恢复失败" description={recoverConnectionMutation.error.message} />
      ) : null}
      {workspaceStatus?.restart_required ? (
        <Alert type="warning" showIcon message={workspaceStatus.restart_notice || '设置已保存，重启 Node 后生效'} />
      ) : null}

      <Typography.Title level={4} style={{ margin: 0 }}>
        设置
      </Typography.Title>

      <Row gutter={[16, 16]}>
        <Col xs={24} xl={14}>
          <Card title="Node 基础配置">
            <Form
              form={form}
              layout="vertical"
              initialValues={settingsQuery.data}
              onFinish={(values) => saveMutation.mutate(values)}
            >
              <Form.Item
                label="节点名称"
                name="name"
                rules={[{ required: true, message: '请输入节点名称' }]}
                extra={defaults?.suggested_name ? `默认使用当前计算机名称：${defaults.suggested_name}` : undefined}
              >
                <Input
                  placeholder={defaults?.suggested_name || '请输入节点名称'}
                  addonAfter={
                    <Button
                      type="link"
                      size="small"
                      onClick={() => {
                        if (defaults?.suggested_name) {
                          form.setFieldValue('name', defaults.suggested_name);
                        }
                      }}
                    >
                      使用本机名称
                    </Button>
                  }
                />
              </Form.Item>

              <Form.Item label="工作区路径" required>
                <Space.Compact style={{ width: '100%' }}>
                  <Form.Item
                    name="workspace_path"
                    noStyle
                    rules={[{ required: true, message: '请选择工作区路径' }]}
                  >
                    <Input placeholder="请选择工作区目录" readOnly />
                  </Form.Item>
                  <Button loading={pickingWorkspace} onClick={() => pickWorkspaceMutation.mutate()}>
                    选择目录
                  </Button>
                </Space.Compact>
              </Form.Item>

              <Form.Item
                label="Hub 地址"
                name="hub_url"
                rules={[{ required: true, message: '请输入 Hub 地址' }]}
              >
                <Input placeholder="ws://localhost:8765/ws" />
              </Form.Item>

              <Form.Item name="require_git_repo" valuePropName="checked">
                <Checkbox>要求工作区必须是 Git 仓库</Checkbox>
              </Form.Item>

              <Form.Item name="watch_workspace" valuePropName="checked">
                <Checkbox>监听工作区变更</Checkbox>
              </Form.Item>

              <Form.Item name="git_protection_enabled" valuePropName="checked">
                <Checkbox>启用 Git 保护</Checkbox>
              </Form.Item>

              <Form.Item name="auto_git_add_new_files" valuePropName="checked">
                <Checkbox>自动 git add 新文件</Checkbox>
              </Form.Item>

              <Typography.Title level={5} style={{ marginBottom: 12 }}>
                通知
              </Typography.Title>

              <Form.Item name="notifications_enabled" valuePropName="checked">
                <Checkbox>启用系统通知</Checkbox>
              </Form.Item>

              <Form.Item name="show_notification_details" valuePropName="checked">
                <Checkbox>通知中显示详细内容</Checkbox>
              </Form.Item>

              <Form.Item
                name="mirror_notifications_to_dingtalk"
                valuePropName="checked"
                extra="开启后，桌面通知会额外通过 Hub 同步到钉钉。"
              >
                <Checkbox>通过 Hub 同步通知到钉钉</Checkbox>
              </Form.Item>

              <Typography.Title level={5} style={{ marginBottom: 12 }}>
                系统集成
              </Typography.Title>

              <Form.Item name="launch_at_login" valuePropName="checked">
                <Checkbox>开机自动启动</Checkbox>
              </Form.Item>

              <Space wrap>
                <Button
                  onClick={async () => {
                    const values = await form.validateFields();
                    validateMutation.mutate({
                      workspace_path: values.workspace_path,
                      require_git_repo: values.require_git_repo,
                    });
                  }}
                  loading={validating}
                >
                  校验工作区
                </Button>
                <Button onClick={() => notificationMutation.mutate()} loading={sendingNotification}>
                  测试通知
                </Button>
                <Button type="primary" htmlType="submit" loading={saving}>
                  保存设置
                </Button>
              </Space>
            </Form>
          </Card>
        </Col>

        <Col xs={24} xl={10}>
          <Space direction="vertical" size={16} style={{ width: '100%' }}>
            <Card title="当前工作区状态" extra={workspaceStatusQuery.isFetching ? '刷新中' : undefined}>
              <Descriptions bordered size="small" column={1}>
                <Descriptions.Item label="有效性">
                  <Tag color={workspaceStatus?.valid ? 'success' : 'error'}>
                    {workspaceStatus?.valid ? '有效' : '无效'}
                  </Tag>
                </Descriptions.Item>
                <Descriptions.Item label="名称">{workspaceStatus?.name || '-'}</Descriptions.Item>
                <Descriptions.Item label="路径">{workspaceStatus?.normalized_path || workspaceStatus?.path || '-'}</Descriptions.Item>
                <Descriptions.Item label="运行中工作区">{workspaceStatus?.running_workspace_path || '-'}</Descriptions.Item>
                <Descriptions.Item label="生效状态">
                  <Tag color={workspaceStatus?.restart_required ? 'warning' : 'success'}>
                    {workspaceStatus?.restart_required ? '需重启生效' : '已生效'}
                  </Tag>
                </Descriptions.Item>
                <Descriptions.Item label="状态说明">{workspaceStatus?.restart_notice || '当前运行状态已与保存配置一致'}</Descriptions.Item>
                <Descriptions.Item label="Git 仓库">
                  <Tag color={workspaceStatus?.git_repo ? 'success' : 'default'}>
                    {workspaceStatus?.git_repo ? '是' : '否'}
                  </Tag>
                </Descriptions.Item>
                <Descriptions.Item label="只读">
                  <Tag color={workspaceStatus?.read_only ? 'warning' : 'success'}>
                    {workspaceStatus?.read_only ? '是' : '否'}
                  </Tag>
                </Descriptions.Item>
                <Descriptions.Item label="监听变更">
                  {workspaceStatus?.watch_workspace ? '开启' : '关闭'}
                </Descriptions.Item>
                <Descriptions.Item label="Git 保护">
                  {workspaceStatus?.git_protection_enabled ? '开启' : '关闭'}
                </Descriptions.Item>
                <Descriptions.Item label="自动 git add">
                  {workspaceStatus?.auto_git_add_new_files ? '开启' : '关闭'}
                </Descriptions.Item>
                <Descriptions.Item label="内部目录">
                  {workspaceStatus?.internal_work_dir || '-'}
                </Descriptions.Item>
                <Descriptions.Item label="错误信息">{workspaceStatus?.error || '-'}</Descriptions.Item>
              </Descriptions>
            </Card>

            <Card
              title="DingTalk 账号绑定"
              extra={
                <Button
                  size="small"
                  icon={<ReloadOutlined />}
                  loading={accountStatusQuery.isFetching}
                  onClick={() => void accountStatusQuery.refetch()}
                >
                  刷新
                </Button>
              }
            >
              <Space direction="vertical" size={12} style={{ width: '100%' }}>
                {accountStatusMissingToken ? (
                  <Alert
                    type="warning"
                    showIcon
                    message="未配置认证令牌，暂时无法使用账号绑定能力"
                    description="请先在 Node 配置中补齐 auth token，再刷新绑定状态。"
                  />
                ) : null}
                <Descriptions bordered size="small" column={1}>
                  <Descriptions.Item label="节点 ID">{accountStatus?.node_id || '-'}</Descriptions.Item>
                  <Descriptions.Item label="绑定能力">
                    <Tag color={accountStatus?.pairing_enabled ? 'success' : 'default'}>
                      {accountStatus?.pairing_enabled ? '已启用' : '未启用'}
                    </Tag>
                  </Descriptions.Item>
                  <Descriptions.Item label="当前绑定用户">
                    {accountStatus?.bound_user_id || '-'}
                  </Descriptions.Item>
                  <Descriptions.Item label="绑定状态">
                    <Tag color={bindingStatusColor(accountStatus, activePairing)}>
                      {bindingStatusLabel(accountStatus, activePairing)}
                    </Tag>
                  </Descriptions.Item>
                  <Descriptions.Item label="绑定码">
                    {activePairing?.pairing_code || '-'}
                  </Descriptions.Item>
                  <Descriptions.Item label="过期时间">
                    {formatUnixTimestamp(activePairing?.expires_at)}
                  </Descriptions.Item>
                  <Descriptions.Item label="说明">
                    {activePairing
                      ? '在钉钉中向 Hub 机器人发送绑定码完成确认。'
                      : accountStatus?.bound_user_id
                        ? '当前节点已绑定钉钉账号，可接收镜像通知。'
                        : '尚未发起绑定。'}
                  </Descriptions.Item>
                </Descriptions>

                <Space wrap>
                  <Button
                    type="primary"
                    onClick={() => startPairingMutation.mutate()}
                    loading={startPairingMutation.isPending}
                    disabled={pairingBusy || !accountStatus?.pairing_enabled}
                  >
                    {activePairing ? '重新生成绑定码' : '发起绑定'}
                  </Button>
                  <Button
                    onClick={() => {
                      if (activePairing) {
                        cancelPairingMutation.mutate({ request_id: activePairing.request_id });
                      }
                    }}
                    loading={cancelPairingMutation.isPending}
                    disabled={pairingBusy || !activePairing}
                  >
                    取消绑定流程
                  </Button>
                  <Button
                    danger
                    onClick={() => deleteBindingMutation.mutate()}
                    loading={deleteBindingMutation.isPending}
                    disabled={pairingBusy || !accountStatus?.bound_user_id}
                  >
                    解绑账号
                  </Button>
                </Space>
              </Space>
            </Card>

            <Card
              title="连接诊断 / 恢复能力"
              extra={
                <Button
                  size="small"
                  icon={<ReloadOutlined />}
                  loading={connectionDiagnosticsQuery.isFetching}
                  onClick={() => void connectionDiagnosticsQuery.refetch()}
                >
                  刷新
                </Button>
              }
            >
              {connectionDiagnostics ? (
                <ConnectionDiagnosticsCardContent
                  diagnostics={connectionDiagnostics}
                  recovering={recoverConnectionMutation.isPending}
                  onRecover={() => recoverConnectionMutation.mutate()}
                />
              ) : connectionDiagnosticsQuery.isLoading || connectionDiagnosticsQuery.isFetching ? (
                <div style={{ textAlign: 'center', padding: 24 }}>
                  <Spin />
                </div>
              ) : (
                <Alert type="info" showIcon message="暂无连接诊断数据" />
              )}
            </Card>

            <Card title="桌面能力状态" extra={capabilityStatusQuery.isFetching ? '刷新中' : undefined}>
              <Descriptions bordered size="small" column={1}>
                <Descriptions.Item label="建议节点名称">{defaults?.suggested_name || '-'}</Descriptions.Item>
                <Descriptions.Item label="系统通知">
                  <Tag color={capabilityStatus?.notifications_enabled ? 'success' : 'default'}>
                    {capabilityStatus?.notifications_enabled ? '开启' : '关闭'}
                  </Tag>
                </Descriptions.Item>
                <Descriptions.Item label="通知详情">
                  <Tag color={capabilityStatus?.show_notification_details ? 'processing' : 'default'}>
                    {capabilityStatus?.show_notification_details ? '显示详情' : '仅标题'}
                  </Tag>
                </Descriptions.Item>
                <Descriptions.Item label="钉钉镜像">
                  <Tag color={capabilityStatus?.mirror_notifications_to_dingtalk ? 'processing' : 'default'}>
                    {capabilityStatus?.mirror_notifications_to_dingtalk ? '已开启' : '未开启'}
                  </Tag>
                </Descriptions.Item>
                <Descriptions.Item label="开机启动">
                  <Tag color={capabilityStatus?.launch_at_login ? 'success' : 'default'}>
                    {capabilityStatus?.launch_at_login ? '已开启' : '未开启'}
                  </Tag>
                </Descriptions.Item>
                <Descriptions.Item label="启动项文件">
                  {capabilityStatus?.launch_agent_path || '-'}
                </Descriptions.Item>
                <Descriptions.Item label="启动项状态">
                  <Tag color={capabilityStatus?.launch_agent_installed ? 'success' : 'default'}>
                    {capabilityStatus?.launch_agent_installed ? '已写入' : '未写入'}
                  </Tag>
                </Descriptions.Item>
              </Descriptions>
            </Card>

            <Card title="最近一次校验">
              {validation ? (
                <Descriptions bordered size="small" column={1}>
                  <Descriptions.Item label="结果">
                    <Tag color={validation.valid ? 'success' : 'error'}>
                      {validation.valid ? '通过' : '失败'}
                    </Tag>
                  </Descriptions.Item>
                  <Descriptions.Item label="工作区名称">{validation.name || '-'}</Descriptions.Item>
                  <Descriptions.Item label="规范路径">{validation.normalized_path || '-'}</Descriptions.Item>
                  <Descriptions.Item label="Git 仓库">
                    {validation.git_repo ? '是' : '否'}
                  </Descriptions.Item>
                  <Descriptions.Item label="错误信息">{validation.error || '-'}</Descriptions.Item>
                </Descriptions>
              ) : (
                <Alert type="info" showIcon message="尚未执行工作区校验" />
              )}
            </Card>
          </Space>
        </Col>
      </Row>
    </Space>
  );
};

function ConnectionDiagnosticsCardContent({
  diagnostics,
  recovering,
  onRecover,
}: {
  diagnostics: DesktopConnectionDiagnostics;
  recovering: boolean;
  onRecover: () => void;
}) {
  return (
    <Space direction="vertical" size={12} style={{ width: '100%' }}>
      <Alert type={connectionOverviewAlertType(diagnostics.overview_state)} showIcon message={diagnostics.overview_message} />
      {diagnostics.recent_error ? (
        <Alert type="error" showIcon message="最近错误" description={diagnostics.recent_error} />
      ) : null}
      <Descriptions bordered size="small" column={1}>
        <Descriptions.Item label="生命周期">
          <Tag color={lifecycleColor(diagnostics.lifecycle_state)}>{diagnostics.lifecycle_state}</Tag>
        </Descriptions.Item>
        <Descriptions.Item label="连接状态">
          <Tag color={connectionStateColor(diagnostics.connection_state)}>{diagnostics.connection_state}</Tag>
        </Descriptions.Item>
        <Descriptions.Item label="Hub 地址">{diagnostics.hub_url}</Descriptions.Item>
        <Descriptions.Item label="节点 ID">{diagnostics.node_id || '-'}</Descriptions.Item>
        <Descriptions.Item label="当前绑定用户">{diagnostics.bound_user_id || '-'}</Descriptions.Item>
        <Descriptions.Item label="认证令牌">
          <Tag color={diagnostics.auth_token_present ? 'success' : 'error'}>
            {diagnostics.auth_token_present ? '已配置' : '缺失'}
          </Tag>
        </Descriptions.Item>
        <Descriptions.Item label="工作区校验">
          <Tag color={diagnostics.workspace_valid ? 'success' : 'error'}>
            {diagnostics.workspace_valid ? '通过' : '失败'}
          </Tag>
        </Descriptions.Item>
        <Descriptions.Item label="工作区错误">{diagnostics.workspace_error || '-'}</Descriptions.Item>
        <Descriptions.Item label="生效状态">
          <Tag color={diagnostics.restart_required ? 'warning' : 'success'}>
            {diagnostics.restart_required ? '需重启生效' : '已生效'}
          </Tag>
        </Descriptions.Item>
        <Descriptions.Item label="状态说明">
          {diagnostics.restart_notice || '当前运行配置与保存配置一致'}
        </Descriptions.Item>
        <Descriptions.Item label="重连间隔">{diagnostics.reconnect_interval_secs} 秒</Descriptions.Item>
        <Descriptions.Item label="最大重连次数">{diagnostics.max_reconnect_attempts}</Descriptions.Item>
      </Descriptions>

      <div>
        <Typography.Text strong>最近日志</Typography.Text>
        <List
          size="small"
          locale={{ emptyText: '暂无日志' }}
          dataSource={diagnostics.recent_logs}
          renderItem={(entry) => (
            <List.Item>
              <Space direction="vertical" size={0} style={{ width: '100%' }}>
                <Space size={8} wrap>
                  <Tag color={logLevelColor(entry.level)}>{entry.level}</Tag>
                  <Typography.Text type="secondary">{formatDateTime(entry.timestamp)}</Typography.Text>
                  <Typography.Text type="secondary">{entry.source}</Typography.Text>
                </Space>
                <Typography.Text>{entry.message}</Typography.Text>
              </Space>
            </List.Item>
          )}
        />
      </div>

      <Space wrap>
        <Button type="primary" onClick={onRecover} loading={recovering}>
          执行恢复
        </Button>
      </Space>
    </Space>
  );
}

function bindingStatusLabel(
  accountStatus?: DesktopAccountStatus,
  pairing?: DesktopPairingRequest | null,
): string {
  if (!accountStatus?.pairing_enabled) {
    return '未启用';
  }
  if (accountStatus.bound_user_id) {
    return '已绑定';
  }
  switch (pairing?.status) {
    case 'pending':
      return '待确认';
    case 'awaiting_confirmation':
      return '等待钉钉确认';
    case 'paired':
      return '已绑定';
    case 'rejected':
      return '已拒绝';
    case 'expired':
      return '已过期';
    case 'cancelled':
      return '已取消';
    default:
      return '未绑定';
  }
}

function bindingStatusColor(
  accountStatus?: DesktopAccountStatus,
  pairing?: DesktopPairingRequest | null,
): string {
  if (!accountStatus?.pairing_enabled) {
    return 'default';
  }
  if (accountStatus.bound_user_id) {
    return 'success';
  }
  switch (pairing?.status) {
    case 'pending':
    case 'awaiting_confirmation':
      return 'processing';
    case 'rejected':
    case 'expired':
    case 'cancelled':
      return 'error';
    default:
      return 'default';
  }
}

function connectionOverviewAlertType(state: string): 'success' | 'info' | 'warning' | 'error' {
  switch (state) {
    case 'bound':
    case 'running':
      return 'success';
    case 'attention':
      return 'warning';
    case 'error':
      return 'error';
    default:
      return 'info';
  }
}

function lifecycleColor(state: string): string {
  switch (state) {
    case 'running':
      return 'success';
    case 'starting':
    case 'stopping':
      return 'processing';
    case 'failed':
      return 'error';
    default:
      return 'default';
  }
}

function connectionStateColor(state: string): string {
  if (state.startsWith('authenticated') || state.startsWith('connected')) {
    return 'success';
  }
  if (state.startsWith('connecting') || state.startsWith('reconnecting') || state.startsWith('authenticating')) {
    return 'processing';
  }
  if (state.startsWith('failed')) {
    return 'error';
  }
  return 'default';
}

function logLevelColor(level: string): string {
  switch (level) {
    case 'INFO':
      return 'blue';
    case 'WARN':
      return 'warning';
    case 'ERROR':
      return 'error';
    default:
      return 'default';
  }
}

function formatDateTime(value?: string | number | null): string {
  if (!value) {
    return '-';
  }

  const timestamp = typeof value === 'number' ? value * 1000 : Date.parse(value);
  if (Number.isNaN(timestamp)) {
    return String(value);
  }

  return new Date(timestamp).toLocaleString();
}

function formatUnixTimestamp(value?: number | null): string {
  if (!value) {
    return '-';
  }

  return new Date(value * 1000).toLocaleString();
}

export default Settings;
