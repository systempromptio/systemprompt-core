use criterion::{Criterion, criterion_group, criterion_main};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::a2a::Task;

fn bench_task_id_generate(c: &mut Criterion) {
    c.bench_function("TaskId::generate", |b| b.iter(|| TaskId::generate()));
}

fn bench_context_id_generate(c: &mut Criterion) {
    c.bench_function("ContextId::generate", |b| b.iter(|| ContextId::generate()));
}

fn bench_message_id_generate(c: &mut Criterion) {
    c.bench_function("MessageId::generate", |b| b.iter(|| MessageId::generate()));
}

fn bench_task_default(c: &mut Criterion) {
    c.bench_function("Task::default", |b| b.iter(Task::default));
}

criterion_group!(
    benches,
    bench_task_id_generate,
    bench_context_id_generate,
    bench_message_id_generate,
    bench_task_default
);
criterion_main!(benches);
