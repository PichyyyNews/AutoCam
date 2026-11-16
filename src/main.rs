mod db;

use db::{BenchmarkResult, Database, DbRecord};
use iced::widget::{button, column, container, mouse_area, row, scrollable, text, Space};
use iced::window;
use iced::{Alignment, Element, Fill, Length, Task, Theme};
use std::sync::{Arc, Mutex};

struct App {
    db: Arc<Mutex<Database>>,
    total_records: usize,
    bench_result: Option<BenchmarkResult>,
    recent_records: Vec<DbRecord>,
    is_benchmarking: bool,
    status_msg: String,
}

#[derive(Debug, Clone)]
enum Message {
    DragWindow,
    MinimizeWindow,
    CloseWindow,
    RunBenchmark(usize),
    BenchmarkFinished(Result<BenchmarkResult, String>),
    RefreshList,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let db = Database::open_optimized("autocam_fast.db")
            .expect("Failed to initialize high-speed SQLite");
        let total = db.count_total().unwrap_or(0);
        let recent = db.fetch_latest(15).unwrap_or_default();
        let db = Arc::new(Mutex::new(db));

        (
            Self {
                db,
                total_records: total,
                bench_result: None,
                recent_records: recent,
                is_benchmarking: false,
                status_msg: "SQLite WAL Mode Ready (Synchronous=NORMAL, MMAP=256MB)".into(),
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DragWindow => window::latest().and_then(window::drag),
            Message::MinimizeWindow => window::latest().and_then(|id| window::minimize(id, true)),
            Message::CloseWindow => window::latest().and_then(window::close),
            Message::RunBenchmark(count) => {
                if self.is_benchmarking {
                    return Task::none();
                }
                self.is_benchmarking = true;
                self.status_msg = format!("Running high-speed batch insert of {count} records...");
                let db_clone = Arc::clone(&self.db);

                // Run DB operations on background thread so UI never stutters
                Task::perform(
                    async move {
                        let mut db = db_clone.lock().map_err(|e| e.to_string())?;
                        db.benchmark_insert(count).map_err(|e| e.to_string())
                    },
                    Message::BenchmarkFinished,
                )
            }
            Message::BenchmarkFinished(result) => {
                self.is_benchmarking = false;
                match result {
                    Ok(bench) => {
                        self.total_records = bench.total_records;
                        self.status_msg = format!(
                            "Inserted {} records in {}ms ({:.0} rec/sec)!",
                            bench.inserted_count, bench.elapsed_ms, bench.records_per_sec
                        );
                        self.bench_result = Some(bench);
                        if let Ok(db) = self.db.lock() {
                            self.recent_records = db.fetch_latest(15).unwrap_or_default();
                        }
                    }
                    Err(err) => {
                        self.status_msg = format!("Benchmark error: {err}");
                    }
                }
                Task::none()
            }
            Message::RefreshList => {
                if let Ok(db) = self.db.lock() {
                    self.recent_records = db.fetch_latest(15).unwrap_or_default();
                    self.total_records = db.count_total().unwrap_or(0);
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // Custom Frameless Window Title Bar
        let title_bar = mouse_area(
            row![
                text("⚡ AutoCam | High-Speed SQLite & Iced (Frameless)")
                    .size(13),
                Space::new().width(Fill),
                button("–")
                    .on_press(Message::MinimizeWindow)
                    .padding([2, 8]),
                button("✕")
                    .on_press(Message::CloseWindow)
                    .padding([2, 8]),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .padding([8, 12]),
        )
        .on_press(Message::DragWindow);

        // Benchmark control buttons
        let bench_buttons = row![
            button(if self.is_benchmarking {
                "Inserting..."
            } else {
                "Insert 10,000 Records"
            })
            .on_press(Message::RunBenchmark(10_000))
            .padding([8, 16]),
            button(if self.is_benchmarking {
                "Inserting..."
            } else {
                "Insert 50,000 Records"
            })
            .on_press(Message::RunBenchmark(50_000))
            .padding([8, 16]),
            button("Refresh List")
                .on_press(Message::RefreshList)
                .padding([8, 16]),
        ]
        .spacing(12);

        // Stats card
        let stats_info = column![
            text(format!("Total Records in DB: {}", self.total_records)).size(16),
            text(format!("Status: {}", self.status_msg)).size(14),
        ]
        .spacing(6);

        // Benchmark metrics display
        let bench_display = if let Some(ref b) = self.bench_result {
            column![
                text("Latest Benchmark Result:").size(14),
                row![
                    text(format!("• Batch Size: {}", b.inserted_count)),
                    text(format!("• Time: {} ms", b.elapsed_ms)),
                    text(format!("• Throughput: {:.0} ops/sec", b.records_per_sec)),
                ]
                .spacing(20),
            ]
            .spacing(4)
        } else {
            column![text("Click a benchmark button above to test SQLite write throughput.").size(13)]
        };

        // Recent records table view
        let records_col = self
            .recent_records
            .iter()
            .fold(column![].spacing(4), |col, rec| {
                col.push(
                    row![
                        text(format!("#{}", rec.id)).width(Length::Fixed(80.0)),
                        text(&rec.tag).width(Length::Fixed(140.0)),
                        text(format!("{:.3}", rec.value)).width(Length::Fixed(120.0)),
                        text(format!("ts: {}", rec.created_at)).width(Length::Fill),
                    ]
                    .spacing(8),
                )
            });

        let records_box = container(scrollable(records_col))
            .style(container::bordered_box)
            .height(Length::Fixed(210.0))
            .padding(10);

        let content = column![
            title_bar,
            column![
                bench_buttons,
                stats_info,
                bench_display,
                text("Recent Records in SQLite:").size(14),
                records_box,
            ]
            .spacing(16)
            .padding(16),
        ];

        container(content)
            .style(container::bordered_box)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .window(window::Settings {
            decorations: false,
            size: iced::Size::new(820.0, 520.0),
            resizable: true,
            ..Default::default()
        })
        .run()
}
