# QGateway

采用websocket模式。实现发送，接收。

## queue模式
- router (路由模式)
- pubsub (懂得都懂)
- rmi (调用模式)

配置方式查看config.yaml

## 客户端
- rust版: https://github.com/QuantRS/qgateway_client
- python版: https://github.com/QuantRS/qgateway_client_pywrap

## 命令请求格式
```
//登陆 认证模式
{"cmdId":0, "args":{"token":"b0107179-42b4-39dd-8220-f48a84b4cef7"}}
//登陆 guest模式
{"cmdId":0, "args":{"token":""}}

//发送消息 (router模式)
{"cmdId":1, "args":{"value": "halo", "key": "呜呜呜", "token":"c09c21f8-c29d-3fb3-86a8-39109742c802"}}
//订阅消息 (router模式) (单key)
{"cmdId":2, "args":{"key": "呜呜呜", "token":"c09c21f8-c29d-3fb3-86a8-39109742c802"}}
//订阅消息 (router模式) (array key)
{"cmdId":2, "args":{"keys": ["呜呜呜"], "token":"c09c21f8-c29d-3fb3-86a8-39109742c802"}}

//发送消息 (pubsub模式)
{"cmdId":1, "args":{"value": "halo", "token":"c09c21f8-c29d-3fb3-86a8-39109742c802"}}
//订阅消息 (pubsub模式)
{"cmdId":2, "args":{"token":"c09c21f8-c29d-3fb3-86a8-39109742c802"}}
```

## 命令cmd_id
```
-1: ping/pong
 0: 校验token arg: {'token': '嘿嘿嘿'}
 1: 发送消息 arg: value: object数据, (选填，看队列模式)key: 我的大名, token: 队列绑定
 2: 接收消息 arg: (选填，看队列模式)key: 我的大名, token: 队列绑定
```
