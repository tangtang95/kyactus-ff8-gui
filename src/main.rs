#![cfg_attr(target_os="windows", windows_subsystem = "windows")]

use async_std::task;
use egui::{Color32, Context};
use kyactus_ff8::library::{
    battle_names::{ENEMY_NAMES, STAGE_NAMES},
    battle_structure::{BattleStructure, Enemy, PackedBattleStructure},
};
use rfd::{AsyncFileDialog, AsyncMessageDialog};
use std::{
    future::Future,
    sync::mpsc::{channel, Receiver, Sender},
};

const BATTLE_STRUCTURE_NUMBER: usize = 1024;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Kyactus - FF8 Battle Structure Editor",
        native_options,
        Box::new(|cc| Ok(Box::new(BattleStructureApp::new(cc)))),
    )
}

pub struct BattleStructureApp {
    file_bytes_channel: (Sender<Vec<u8>>, Receiver<Vec<u8>>),
    battle_structure_list: Vec<BattleStructure>,
    battle_structure_index: usize,
    enemy_selected_index: usize,
}

impl BattleStructureApp {
    /// Called once before the first frame.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            file_bytes_channel: channel(),
            battle_structure_list: Vec::new(),
            battle_structure_index: 0,
            enemy_selected_index: 0,
        }
    }
}

impl eframe::App for BattleStructureApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if let Ok(bytes) = self.file_bytes_channel.1.try_recv() {
            match read_battle_structures(&bytes) {
                Ok(battle_structure_list) => {
                    self.battle_structure_list = battle_structure_list;
                    self.battle_structure_index = 0;
                    self.enemy_selected_index = 0;
                }
                Err(err) => {
                    execute(async move {
                        error_dialog(&err.to_string()).await;
                    });
                }
            }
        }

        egui::TopBottomPanel::top("app_top_bar")
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(4.0))
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        ui.set_max_width(200.0);

                        if ui.button("Open...").clicked() {
                            let sender = self.file_bytes_channel.0.clone();
                            let task = AsyncFileDialog::new()
                                .set_title("Select scene.out file")
                                .add_filter("scene.out", &["out"])
                                .set_directory(".")
                                .pick_file();
                            let ctx = ui.ctx().clone();
                            execute(async move {
                                let file = task.await;
                                if let Some(file) = file {
                                    let bytes = file.read().await;
                                    let _ = sender.send(bytes);
                                    ctx.request_repaint();
                                }
                            });
                            ui.close_menu();
                        }

                        let save_as_enabled = !self.battle_structure_list.is_empty();
                        if ui
                            .add_enabled(save_as_enabled, egui::Button::new("Save as..."))
                            .clicked()
                        {
                            let task = rfd::AsyncFileDialog::new().save_file();
                            match write_packed_battle_structure(&self.battle_structure_list) {
                                Ok(contents) => {
                                    execute(async move {
                                        let file = task.await;
                                        if let Some(file) = file {
                                            _ = file.write(contents.as_ref()).await;
                                        }
                                    });
                                }
                                Err(err) => {
                                    execute(async move {
                                        error_dialog(&err.to_string()).await;
                                    });
                                }
                            };
                            ui.close_menu();
                        }
                    });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.battle_structure_list.is_empty() {
                ui.heading("Battle Structure");
                frame().show(ui, |ui| {
                    ui.label("Encounter ID");
                    ui.add(egui::Slider::new(
                        &mut self.battle_structure_index,
                        0..=BATTLE_STRUCTURE_NUMBER - 1,
                    ));
                });
                ui.separator();

                match self
                    .battle_structure_list
                    .get_mut(self.battle_structure_index)
                {
                    Some(battle_structure) => {
                        ui.heading("Stage");
                        frame().show(ui, |ui| stage_contents(ui, battle_structure));
                        ui.separator();
                        ui.heading("Enemies");
                        frame().show(ui, |ui| {
                            enemies_contents(ui, battle_structure, &mut self.enemy_selected_index)
                        });
                        ui.separator();
                    }
                    None => {
                        ui.heading("Battle structure not found!");
                    }
                }
            } else {
                ui.centered_and_justified(|ui| {
                    ui.heading("Open a scene.out file to start editing it");
                });
            }
        });
    }
}

fn stage_contents(ui: &mut egui::Ui, battle_structure: &mut BattleStructure) {
    egui::ComboBox::from_label("Battle stage")
        .selected_text(
            STAGE_NAMES
                .get(battle_structure.stage_id as usize)
                .unwrap_or(&"Invalid Stage Id!")
                .to_string(),
        )
        .show_ui(ui, |ui| {
            (0..STAGE_NAMES.len()).for_each(|i| {
                ui.selectable_value(&mut battle_structure.stage_id, i as u8, STAGE_NAMES[i]);
            });
        });
    ui.add_space(16.0);
    ui.columns(2, |cols| {
        cols[0].vertical(|ui| {
            ui.heading("Flags");
            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    ui.checkbox(&mut battle_structure.flags.cannot_escape, "Cannot escape");
                    ui.checkbox(&mut battle_structure.flags.no_exp, "No exp gained");
                    ui.checkbox(
                        &mut battle_structure.flags.scripted_battle,
                        "Scripted battle",
                    );
                    ui.checkbox(&mut battle_structure.flags.show_timer, "Show timer");
                });

                cols[1].vertical(|ui| {
                    ui.checkbox(
                        &mut battle_structure.flags.force_back_attack,
                        "Force back attack",
                    );
                    ui.checkbox(
                        &mut battle_structure.flags.force_surprise_attack,
                        "Force surprise attack",
                    );
                    ui.checkbox(
                        &mut battle_structure.flags.disable_win_fanfare,
                        "Disable victory fanfare",
                    );
                    ui.checkbox(
                        &mut battle_structure.flags.disable_exp_screen,
                        "Do not show exp. screen",
                    );
                });
            })
        });
        cols[1].vertical(|ui| {
            ui.heading("Camera");
            ui.add(
                egui::Slider::new(&mut battle_structure.main_camera.number, 0..=3)
                    .text("Main camera number"),
            );
            ui.add(
                egui::Slider::new(&mut battle_structure.main_camera.animation, 0..=7)
                    .text("Main camera animation"),
            );
            ui.add(
                egui::Slider::new(&mut battle_structure.secondary_camera.number, 0..=3)
                    .text("Secondary camera number"),
            );
            ui.add(
                egui::Slider::new(&mut battle_structure.secondary_camera.animation, 0..=7)
                    .text("Secondary camera animation"),
            );
        });
    });
}

fn enemies_contents(
    ui: &mut egui::Ui,
    battle_structure: &mut BattleStructure,
    enemy_selected_index: &mut usize,
) {
    ui.columns(2, |cols| {
        cols[0].vertical(|ui| {
            for i in 0..battle_structure.enemies.len() {
                let enemy = &battle_structure.enemies[i];
                let enemy_name = format!(
                    "{i}. {}",
                    *ENEMY_NAMES
                        .get(enemy.id as usize)
                        .unwrap_or(&"Invalid enemy name!")
                );

                let enemy_name = if enemy.enabled {
                    enemy_name.to_string()
                } else {
                    enemy_name + " (disabled)"
                };

                let text_color = if enemy.enabled {
                    Color32::PLACEHOLDER
                } else {
                    Color32::DARK_GRAY
                };

                ui.selectable_value(
                    enemy_selected_index,
                    i,
                    egui::RichText::new(enemy_name).color(text_color),
                );
            }
        });

        cols[1].vertical(
            |ui| match battle_structure.enemies.get_mut(*enemy_selected_index) {
                Some(enemy) => {
                    enemy_contents(ui, enemy);
                }
                None => {
                    ui.heading("Enemy not found!");
                }
            },
        );
    })
}

fn enemy_contents(ui: &mut egui::Ui, enemy: &mut Enemy) {
    egui::ComboBox::from_label("Enemy")
        .selected_text(
            ENEMY_NAMES
                .get(enemy.id as usize)
                .unwrap_or(&"Invalid enemy id!")
                .to_string(),
        )
        .show_ui(ui, |ui| {
            (0..ENEMY_NAMES.len()).for_each(|i| {
                ui.selectable_value(&mut enemy.id, i as u8, ENEMY_NAMES[i]);
            });
        });
    ui.add(egui::Slider::new(&mut enemy.level, 0..=255).text("Level"));
    ui.checkbox(&mut enemy.enabled, "Enabled");
    ui.checkbox(&mut enemy.not_loaded, "NOT loaded");
    ui.checkbox(&mut enemy.invisible, "NOT visible");
    ui.checkbox(&mut enemy.untargetable, "NOT targetable");

    ui.add_space(8.0);
    ui.label("Coordinates");
    ui.add(egui::Slider::new(&mut enemy.coordinate.x, i16::MIN..=i16::MAX).text("X"));
    ui.add(egui::Slider::new(&mut enemy.coordinate.y, i16::MIN..=i16::MAX).text("Y"));
    ui.add(egui::Slider::new(&mut enemy.coordinate.z, i16::MIN..=i16::MAX).text("Z"));

    ui.collapsing("Advanced options", |ui| {
        ui.add(
            egui::Slider::new(&mut enemy.unknown_1, 0..=u16::MAX)
                .text("Unknown 1")
                .hexadecimal(1, false, true),
        );
        ui.add(
            egui::Slider::new(&mut enemy.unknown_2, 0..=u16::MAX)
                .text("Unknown 2")
                .hexadecimal(1, false, true),
        );
        ui.add(
            egui::Slider::new(&mut enemy.unknown_3, 0..=u16::MAX)
                .text("Unknown 3")
                .hexadecimal(1, false, true),
        );
        ui.add(
            egui::Slider::new(&mut enemy.unknown_4, 0..=u8::MAX)
                .text("Unknown 4")
                .hexadecimal(1, false, true),
        );
    });
}

fn frame() -> egui::Frame {
    egui::Frame::none()
        .inner_margin(8.0)
        .outer_margin(4.0)
}

fn error_dialog(message: &str) -> impl Future<Output = rfd::MessageDialogResult> {
    AsyncMessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_buttons(rfd::MessageButtons::Ok)
        .set_title("Error")
        .set_description(message)
        .show()
}

fn execute<F: Future<Output = ()> + Send + 'static>(f: F) {
    task::spawn(f);
}

fn read_battle_structures(bytes: &[u8]) -> anyhow::Result<Vec<BattleStructure>> {
    if bytes.len() != size_of::<PackedBattleStructure>() * BATTLE_STRUCTURE_NUMBER {
        return Err(anyhow::anyhow!("Incorrect bytes size"));
    }

    let mut battle_structure_list = Vec::with_capacity(BATTLE_STRUCTURE_NUMBER);
    for i in 0..BATTLE_STRUCTURE_NUMBER {
        let offset = i * size_of::<PackedBattleStructure>();
        let packed_bs = PackedBattleStructure::try_from_bytes(
            bytes
                .get(offset..offset + size_of::<PackedBattleStructure>())
                .ok_or(anyhow::anyhow!(
                    "Could not retrieve data. File size not as expected!"
                ))?,
        )?;
        battle_structure_list.push(packed_bs.into_battle_structure());
    }
    Ok(battle_structure_list)
}

fn write_packed_battle_structure(
    battle_structure_list: &Vec<BattleStructure>,
) -> anyhow::Result<Vec<u8>> {
    let mut bytes: Vec<u8> =
        Vec::with_capacity(BATTLE_STRUCTURE_NUMBER * size_of::<PackedBattleStructure>());

    if battle_structure_list.len() != BATTLE_STRUCTURE_NUMBER {
        return Err(anyhow::anyhow!(format!(
            "Battle structure size is incorrect: {}",
            battle_structure_list.len()
        )));
    }

    for battle_structure in battle_structure_list {
        bytes.extend_from_slice(battle_structure.as_packed_bytes()?.as_ref());
    }
    Ok(bytes)
}
