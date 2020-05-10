use base64::{CharacterSet, Config};
use directories::ProjectDirs;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::hash;
use sodiumoxide::crypto::sign;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

mod replicant;
use replicant::{
    create_account, create_crdt, create_crdt_info, get_random_id, Account, Applyable, CRDTInfo,
    Counter, Nat, Operation, OperationSigned, UserPubKey, UserSecKey, CRDT,
};

use ansi_term::Colour::Red;

fn base64_config() -> Config {
    Config::new(CharacterSet::UrlSafe, false)
}

fn main() {
    let _ = ansi_term::enable_ansi_support();
    let args: Vec<String> = env::args().collect();

    if args.len() >= 2 {
        let project_name: &str = &args[1];
        let project_basedir_str = format!("{}/", project_name);
        let project_file_str = format!("project.penny");
        let project_basedir = std::path::Path::new(&project_basedir_str);
        let pennyfile_dir = project_basedir.join(std::path::Path::new(&project_file_str));

        match File::open(&pennyfile_dir) {
            Ok(mut file) => {
                println!("Looking for a project at {:?}.", pennyfile_dir);
                let mut contents = vec![];
                file.read_to_end(&mut contents).unwrap();
                let project_info: CRDTInfo<Nat> = bincode::deserialize(&contents).unwrap();

                let DirectoryLevelUserInfo { pk, sk, .. } = get_keypair(&pennyfile_dir);
                let account = create_account(pk, sk);

                let crdt = create_crdt(project_info);
                let crdt = restore_operations::<Nat>(crdt, project_basedir);

                println!("Testing the {} CRDT", Nat::NAME);
                run(crdt, account, project_basedir);
            }
            Err(_) => {
                print!(
                    "Couldn't open '{}'! Do you want me to create it? ",
                    project_name
                );
                io::stdout().flush().unwrap();
                let mut contents = String::new();
                io::stdin().read_line(&mut contents).unwrap();
                if contents.trim() == "y" {
                    let info: CRDTInfo<Nat> = create_crdt_info(Nat::from(0), get_random_id());
                    let info =
                        bincode::serialize(&info).expect("somehow there was a serialization error");
                    let _test: CRDTInfo<Nat> = bincode::deserialize(&info).unwrap();
                    fs::create_dir_all(project_basedir).unwrap();
                    {
                        let mut project_file = File::create(&pennyfile_dir).unwrap();
                        project_file.write_all(&info).unwrap();
                    }
                    println!("I created a new project at {:?}.", pennyfile_dir);
                }
            }
        }
    } else {
        println!("Input the name of the project");
    }
}

fn run(mut crdt: CRDT<Nat>, mut account: Account, project_basedir: &Path) {
    loop {
        println!(
            "Current value: {}",
            Red.paint(format!("{}", crdt.value.value))
        );
        print!("Increment: ");
        io::stdout().flush().unwrap();
        let mut increment = String::new();
        io::stdin().read_line(&mut increment).unwrap();
        match increment.trim().parse() {
            Ok(increment) => {
                crdt = crdt.apply_desc(&mut account, increment);
            }
            _ => break,
        }
    }
    save_operations::<Nat>(crdt.flush(), project_basedir);
}

fn restore_operations<T>(crdt: CRDT<T>, project_basedir: &Path) -> CRDT<T>
where
    T: Applyable + Serialize + DeserializeOwned,
    T::Description: Serialize + DeserializeOwned + Ord,
{
    let operation_dir = project_basedir.join("operations");
    let mut all_operations: Vec<Operation<T::Description>> = vec![];
    if operation_dir.exists() {
        for user_entry in fs::read_dir(&operation_dir).expect(&format!(
            "Trying to read the '{}' folder, but couldn't open it for whatever reason",
            operation_dir.to_string_lossy()
        )) {
            let user_entry = user_entry.expect(&format!(
                "ran into an error when reading an entry in the '{}' folder",
                operation_dir.to_string_lossy()
            ));

            let path = user_entry.path();

            if path.is_dir() {
                all_operations.extend(get_operations_in_path::<T>(&path));
            } else {
                panic!(
                    "I only expected directories in {}, but I came across {}, which is a file!",
                    operation_dir.to_string_lossy(),
                    path.to_string_lossy()
                );
            }
        }
        all_operations.into_iter().fold(crdt, CRDT::apply)
    } else {
        crdt
    }
}

fn get_operations_in_path<T>(base_path: &PathBuf) -> Vec<Operation<T::Description>>
where
    T: Applyable + DeserializeOwned,
    T::Description: DeserializeOwned,
{
    let user_pub_key: UserPubKey = {
        let user_pub_key = base_path.components().into_iter().last().unwrap();
        let user_pub_key = match user_pub_key {
            std::path::Component::Normal(osstr) => osstr.to_string_lossy(),
            _ => panic!(
                "The last element of {} wasn't a normal part of a path",
                base_path.to_string_lossy()
            ),
        };
        let user_pub_key_decoded = base64::decode_config(user_pub_key.as_bytes(), base64_config())
            .expect(&format!("{} couldn't be decoded as base64!", user_pub_key));

        bincode::deserialize(&user_pub_key_decoded).expect(&format!(
            "{} couldn't be converted to a valid public key!",
            user_pub_key
        ))
    };

    fs::read_dir(&base_path)
        .expect(&format!(
            "Trying to read the '{}' folder, but couldn't open it for whatever reason",
            base_path.to_string_lossy()
        ))
        .map(|operation| {
            let operation_signed: OperationSigned<T::Description> = {
                let mut operation_bytes = vec![];
                let operation_path = operation.unwrap().path();
                let mut file = OpenOptions::new()
                    .read(true)
                    .write(false)
                    .create(false)
                    .open(&operation_path)
                    .unwrap();
                file.read_to_end(&mut operation_bytes).unwrap();
                bincode::deserialize(&operation_bytes).expect(&format!(
                    "The file at {} couldn't be decoded into a valid operation!",
                    operation_path.to_string_lossy()
                ))
            };
            let operation = Operation {
                user_pub_key,
                data: operation_signed,
            };
            operation
        })
        .collect()
}

fn save_operations<T>(
    mut operations: HashMap<Counter, Operation<T::Description>>,
    project_basedir: &Path,
) where
    T: Applyable + Serialize,
    T::Description: Serialize,
{
    for (counter, operation) in operations.drain() {
        let to_write_dir = {
            let relative_dir = format!(
                "operations/{}",
                base64::encode_config(
                    bincode::serialize(&operation.user_pub_key).unwrap(),
                    base64_config()
                )
            );
            project_basedir.join(std::path::Path::new(&relative_dir))
        };
        fs::create_dir_all(&to_write_dir).expect("Failed to create directory to store operations");
        let to_write_file_path =
            to_write_dir.join(std::path::Path::new(&format!("{}.pennyop", counter)));
        if to_write_file_path.exists() {
            panic!("Something is messed up... I want to write to {} but it already exists. That's bad! Aborting", to_write_file_path.to_string_lossy());
        }
        let mut file = OpenOptions::new()
            .read(false)
            .write(true)
            .create(true)
            .open(to_write_file_path)
            .unwrap();
        file.write_all(
            &bincode::serialize(&operation.data).expect("somehow there was a serialization error"),
        )
        .expect("Failed to write operation");
    }
}

// This contains the information needed to create new operations on the CRDT.
// It is NOT needed to read the operations. It should stay private.
// Opening the same project in two different directories will result in different UserInfos.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
struct DirectoryLevelUserInfo {
    pk: UserPubKey,
    sk: UserSecKey,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
struct ComputerLevelUserInfo {
    computer_pk: UserPubKey,
    computer_sk: UserSecKey,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
struct SavedKeys {
    computer_level_user_info: ComputerLevelUserInfo,
    dir_level_keys: HashMap<String, DirectoryLevelUserInfo>,
}

fn get_keypair(pennyfile_dir: &PathBuf) -> DirectoryLevelUserInfo {
    let pennyfile_dir_hash_string = {
        let pennyfile_dir_canonicalized = fs::canonicalize(pennyfile_dir).unwrap();
        let pennyfile_dir_bytes = pennyfile_dir_canonicalized
            .to_str()
            .expect(
                "The path the penny file is on isn't valid unicode, that is a requirement for now.",
            )
            .as_bytes();
        let pennyfile_dir_hash = hash::hash(pennyfile_dir_bytes);
        base64::encode_config(pennyfile_dir_hash, base64_config())
    };

    let mut keys = get_all_saved_keypairs();
    let dir_keypair = keys
        .dir_level_keys
        .entry(pennyfile_dir_hash_string)
        .or_insert_with(|| {
            let (pk, sk) = sign::gen_keypair();
            DirectoryLevelUserInfo { pk, sk }
        });
    let dir_keypair = dir_keypair.clone(); // I feel like there should be a way not to have to clone here
    set_all_saved_keypairs(&keys);
    dir_keypair
}

fn get_all_saved_keypairs() -> SavedKeys {
    // @todo: generate different keypairs for different directories
    if let Some(proj_dirs) = ProjectDirs::from("com", "PennySoftware", "Replicant") {
        let config_dir = proj_dirs.config_dir();
        println!("Config directory is {:?}", &config_dir);

        fs::create_dir_all(config_dir).expect("Failed to create configuration directory");
        let keys_path = config_dir.join(std::path::Path::new("keys.json"));
        match File::open(&keys_path) {
            Ok(mut file) => {
                let mut contents = String::new();
                file.read_to_string(&mut contents).unwrap();
                let keys: SavedKeys = serde_json::from_str(&contents).unwrap();
                keys
            }
            Err(_) => {
                let (pk, sk) = sign::gen_keypair();
                let keys = SavedKeys {
                    computer_level_user_info: ComputerLevelUserInfo {
                        computer_pk: pk,
                        computer_sk: sk,
                    },
                    dir_level_keys: HashMap::new(),
                };

                let mut file = File::create(keys_path).unwrap();
                write!(file, "{}", serde_json::to_string(&keys).unwrap()).unwrap();
                keys
            }
        }
    } else {
        panic!("couldn't get the project directory!")
    }
}

fn set_all_saved_keypairs(keys: &SavedKeys) {
    // @todo: generate different keypairs for different directories
    if let Some(proj_dirs) = ProjectDirs::from("com", "PennySoftware", "Replicant") {
        let config_dir = proj_dirs.config_dir();
        println!("Config directory is {:?}", &config_dir);

        fs::create_dir_all(config_dir).expect("Failed to create configuration directory");
        let keys_path = config_dir.join(std::path::Path::new("keys.json"));

        let mut file = OpenOptions::new()
            .read(false)
            .write(true)
            .create(true)
            .open(keys_path)
            .unwrap();

        write!(file, "{}", serde_json::to_string(keys).unwrap()).unwrap();
    } else {
        panic!("couldn't get the project directory!")
    };
}
