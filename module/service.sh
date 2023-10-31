#!/system/bin/sh
#
# Copyright 2023 shadow3aaa@gitbub.com
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
#  You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
MODDIR=${0%/*}
DIR=/data/media/0/Android/fas-rs
UPDATE_CONF=$MODDIR/update_games.toml

# update vtools support
sh $MODDIR/vtools/init_vtools.sh $(realpath $MODDIR/module.prop)

# wait until the sdcard is decrypted
until [ -d $DIR ]; do
	sleep 1
done

# update config
if [ -f $UPDATE_CONF ]; then
	mv $UPDATE_CONF $DIR/games.toml
fi

# start with user profile
killall fas-rs
nohup env FAS_LOG=info $MODDIR/fas-rs --run --local-profile $DIR/games.toml --std-profile $MODDIR/games.toml >$DIR/fas_log.txt 2>&1 &
