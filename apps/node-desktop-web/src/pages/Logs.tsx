import { Card, List, Tag, Typography } from 'antd';
import { logs } from '../mock/data';

const levelColor: Record<string, string> = {
  INFO: 'blue',
  WARN: 'orange',
  ERROR: 'red',
};

const Logs: React.FC = () => {
  return (
    <Card title="日志中心">
      <List
        dataSource={logs}
        renderItem={(entry) => (
          <List.Item>
            <List.Item.Meta
              title={
                <Typography.Text>
                  <Tag color={levelColor[entry.level]}>{entry.level}</Tag>
                  {entry.message}
                </Typography.Text>
              }
              description={entry.timestamp}
            />
          </List.Item>
        )}
      />
    </Card>
  );
};

export default Logs;
