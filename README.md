# QGateway

采用websocket模式。实现发送，接收。

## 命令请求格式
```
//有序args
{'cmd_id': 'xx', args: []}
//DIY args
{'cmd_id': 'xx', args: {
    "嘿嘿": "嘿嘿嘿"
    ...
}}
```

## 命令cmd_id
```
-1: ping/pong
 0: 校验token arg: {'token': '嘿嘿嘿'}
 1: 发送消息 arg: value: object数据, (选填，看队列模式)key: 我的大名, token: 队列绑定
 2: 接收消息 arg: (选填，看队列模式)key: 我的大名, token: 队列绑定
```