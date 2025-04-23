## 功能总结

### 迁移系统调用与 TCB 内容以适配新的进程结构

尽管本章的进程结构有所更新，但核心结构基本未变，因此迁移工作较为容易。

- 已迁移系统调用函数：  
  `sys_spawn`、`sys_get_time`、`sys_mmap`、`sys_munmap`，其结构与逻辑未做修改。

- `TaskControlBlock (TCB)` 中的以下字段已完成迁移：  
  - `prio`（优先级）  
  - `pass`（步进器累计值）  
  - `stride`（步长）

### 新增系统调用实现

- `sys_fstat`：获取文件状态信息  
- `sys_linkat`：创建硬链接  
- `sys_unlinkat`：删除文件或链接

---

## 问答题

### ch6

在 `easy-fs` 中，`Inode` 结构是磁盘中某个 `DiskInode` 的抽象表示，它通过 `block_id` 和 `block_offset` 来定位具体的 `inode`。

挂载文件系统后，系统通过 `root inode` 访问其下的文件或目录。如果 `root inode` 的信息损坏（如 `block_id`、`block_offset`）或其对应的 `DiskInode` 内容异常，将可能导致以下问题：

- 根目录下的文件查找失败  
- 无法访问数据  
- 文件系统回收机制异常，产生数据泄漏

### ch7

#### 使用 `pipe` 的一个实际应用示例为：

```bash
ps aux | grep firefox
```
此命令将 `ps aux`（查看所有进程）输出的结果通过管道 `|` 传递给 `grep firefox`，用于筛选包含 “`firefox`” 的行。这里 `pipe` 在内核中连接了两个进程的标准输出和标准输入，实现数据流的传递。

#### 一个更易用的多进程通信机制

##### 设计目标

- 实现多个发送者与多个接收者之间的通信。
- 每个消息可以定向给特定接收进程。
- 不需要为每一对通信进程手动建立管道。
- 支持进程动态注册和注销。
- 支持阻塞与非阻塞读取。

---

##### 数据结构定义

```rust
/// 单条消息结构体
struct Message {
    sender_pid: usize, // 发送方的 pid
    receiver_pid: usize, // 接收方的 pid
    len: usize, // 消息长度
    data: [u8; MAX_MSG_LEN], // 消息缓存区
}

/// 一个消息队列（用于多进程共享）
struct MessageQueue {
    key: usize, // 标识符
    queue: VecDeque<Message>, 
    registered_receivers: HashSet<usize>, 
    registered_senders: HashSet<usize>,
}
```
1. 消息结构（Message）

消息是进程间传递的基本单位。每条消息都包含了以下内容：

    sender_pid: 发送进程的进程ID（PID）。该字段用于标识消息的发送者。

    receiver_pid: 接收进程的进程ID（PID）。该字段用于标识消息的接收者。

    len: 消息的有效数据长度，单位为字节。

    data: 存储消息内容的字节数组，最大长度为 MAX_MSG_LEN。该字段存储实际要传递的数据。

这种消息结构可以帮助我们在发送和接收过程中传递相关的元数据（如发送者和接收者的 PID）以及消息本身的数据。

2. 消息队列（MessageQueue）

消息队列是用于存储多个消息并支持多个进程间共享的核心结构。每个消息队列包含以下字段：

    key: 唯一标识一个消息队列的标识符。通过这个 key，系统可以区分不同的消息队列。

    queue: 一个 VecDeque<Message> 类型的队列，存储所有待发送和待接收的消息。使用双端队列（VecDeque）可以实现高效的消息进出队列操作。

    registered_receivers: 一个 HashSet<usize>，记录已经注册为接收者的进程的 PID。只有注册的接收进程才有权限从该队列接收消息。

    registered_senders: 一个 HashSet<usize>，记录已经注册为发送者的进程的 PID。只有注册的发送进程才可以向该队列发送消息。

消息队列的设计使得多个进程可以共享同一个队列进行通信，而不必为每一对进程创建独立的管道。
##### 核心方法定义
```rust
impl MessageQueue {
    /// 创建一个新的消息队列
    pub fn new(key: usize) -> Self {
        Self {
            key,
            queue: VecDeque::new(),
            registered_receivers: HashSet::new(),
            registered_senders: HashSet::new(),
        }
    }

    /// 注册发送进程
    pub fn register_sender(&mut self, pid: usize) {
        self.registered_senders.insert(pid);
    }

    /// 注册接收进程
    pub fn register_receiver(&mut self, pid: usize) {
        self.registered_receivers.insert(pid);
    }

    /// 发送消息（必须指定接收方）
    pub fn send(&mut self, msg: Message) -> Result<(), &'static str> {
        if !self.registered_senders.contains(&msg.sender_pid) {
            return Err("Sender not registered");
        }
        if !self.registered_receivers.contains(&msg.receiver_pid) {
            return Err("Receiver not registered");
        }
        self.queue.push_back(msg);
        Ok(())
    }

    /// 接收进程接收消息（从队列中查找目标消息）
    pub fn recv(&mut self, my_pid: usize) -> Option<Message> {
        if !self.registered_receivers.contains(&my_pid) {
            return None;
        }
        if let Some(pos) = self.queue.iter().position(|m| m.receiver_pid == my_pid) {
            Some(self.queue.remove(pos).unwrap())
        } else {
            None
        }
    }
}
```
核心方法

以下是对消息队列的一些基本操作方法：

    new(key: usize): 创建一个新的消息队列，给定一个唯一的 key 标识符。初始化时，队列为空，没有注册的发送者和接收者。

    register_sender(pid: usize): 注册发送进程，通过将进程的 PID 添加到 registered_senders 集合中，允许该进程向消息队列发送消息。

    register_receiver(pid: usize): 注册接收进程，通过将进程的 PID 添加到 registered_receivers 集合中，允许该进程从消息队列接收消息。

    send(msg: Message): 发送消息。调用该方法时，发送进程必须已经注册，并且接收进程必须存在且已注册。若消息的发送方或接收方未注册，则返回错误信息。消息成功添加到队列后，发送成功。

    recv(my_pid: usize): 接收消息。接收进程通过提供其 PID 来查询消息队列中是否有匹配的消息（即 receiver_pid 与进程 PID 匹配的消息）。如果队列中存在可用的消息，则返回消息，并从队列中移除该消息；如果没有匹配的消息，返回 None。

---
## 荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

         使用了 Chatgpt 交流了有关汇编与Rust语法上的问题
         借助了 AI 助教理解实验框架

2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

         《rCore-Tutorial-Guide-2025S》
			《rCore-Tutorial-Book-v3》
         《Operating Systems: Three Easy Pieces》 Remzi H. Arpaci-Dusseau and Andrea C. Arpaci-Dusseau

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

