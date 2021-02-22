/*
 * Copyright (C) 2010 Red Hat, Inc.
 *
 * Author: Steven Dake <sdake@redhat.com>
 *
 * This file is part of libqb.
 *
 * libqb is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Lesser General Public License as published by
 * the Free Software Foundation, either version 2.1 of the License, or
 * (at your option) any later version.
 *
 * libqb is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public License
 * along with libqb.  If not, see <http://www.gnu.org/licenses/>.
 */
#include "os_base.h"

#include <qb/qbmap.h>
#include <pthread.h>
#include "util_int.h"
#include "map_int.h"
#include <qb/qblist.h>

#define FNV_32_PRIME ((uint32_t)0x01000193)

struct hash_node {
	struct qb_list_head list;
	void *value;
	const char *key;
	uint32_t refcount;
	struct qb_list_head notifier_head;
};

struct hash_bucket {
	struct qb_list_head list_head;
	pthread_mutex_t bkt_lock;
};

struct hash_table {
	struct qb_map map;
	size_t count;
	uint32_t order;
	uint32_t hash_buckets_len;
	struct qb_list_head notifier_head;
	/* Lock ordering:
	   If you need a bucket lock and this lock, always get the bucket lock first */
	pthread_mutex_t ht_lock;
	struct hash_bucket hash_buckets[0];
};

struct hashtable_iter {
	struct qb_map_iter i;
	struct hash_node *node;
	uint32_t bucket;
};

static struct qb_list_head *copy_notify_list(struct hash_table *t, struct hash_node *n, uint32_t event);
static void hashtable_notify(struct qb_list_head *head, const char *key,
			     void *old_value, void *value);
static void hashtable_node_deref_under_bucket(struct qb_map *map,
					      int32_t hash_entry);
static void hashtable_node_destroy(struct hash_table *t,
				   struct hash_node *hash_node);

static uint32_t
hash_fnv(const void *value, uint32_t valuelen, uint32_t order)
{
	uint8_t *cd = (uint8_t *) value;
	uint8_t *ed = (uint8_t *) value + valuelen;
	uint32_t hash_result = 0x811c9dc5;
	int32_t res;

	while (cd < ed) {
		hash_result ^= *cd;
		hash_result *= FNV_32_PRIME;
		cd++;
	}
	res = ((hash_result >> order) ^ hash_result) & ((1 << order) - 1);
	return (res);
}

static uint32_t
qb_hash_string(const void *key, uint32_t order)
{
	char *str = (char *)key;
	return hash_fnv(key, strlen(str), order);
}

static struct hash_node *
hashtable_lookup(struct hash_table *t, const char *key)
{
	uint32_t hash_entry;
	struct qb_list_head *list;
	struct hash_node *hash_node;

	hash_entry = qb_hash_string(key, t->order);
	if (pthread_mutex_lock(&t->hash_buckets[hash_entry].bkt_lock)) {
		return NULL;
	}

	qb_list_for_each(list, &t->hash_buckets[hash_entry].list_head) {

		hash_node = qb_list_entry(list, struct hash_node, list);
		if (strcmp(hash_node->key, key) == 0) {
			pthread_mutex_unlock(&t->hash_buckets[hash_entry].bkt_lock);
			return hash_node;
		}
	}
	pthread_mutex_unlock(&t->hash_buckets[hash_entry].bkt_lock);
	return NULL;
}

/* As above but returns the value so we don't need to deref the node
 * outside the lock
 */
static void *
hashtable_lookup_value(struct hash_table *t, const char *key)
{
	uint32_t hash_entry;
	struct qb_list_head *list;
	struct hash_node *hash_node;

	hash_entry = qb_hash_string(key, t->order);
	if (pthread_mutex_lock(&t->hash_buckets[hash_entry].bkt_lock)) {
		return NULL;
	}

	qb_list_for_each(list, &t->hash_buckets[hash_entry].list_head) {

		hash_node = qb_list_entry(list, struct hash_node, list);
		if (strcmp(hash_node->key, key) == 0) {
			void *value = hash_node->value;
			pthread_mutex_unlock(&t->hash_buckets[hash_entry].bkt_lock);
			return value;
		}
	}
	pthread_mutex_unlock(&t->hash_buckets[hash_entry].bkt_lock);
	return NULL;
}

static void *
hashtable_get(struct qb_map *map, const char *key)
{
	struct hash_table *t = (struct hash_table *)map;
	return hashtable_lookup_value(t, key);
}

static void
hashtable_node_destroy(struct hash_table *t, struct hash_node *hash_node)
{
	struct qb_list_head *pos;
	struct qb_list_head *next;
	struct qb_map_notifier *tn;
	struct qb_list_head *nl;

	nl = copy_notify_list(t, hash_node, QB_MAP_NOTIFY_DELETED);
	hashtable_notify(nl,
			 hash_node->key, hash_node->value, NULL);

	qb_list_for_each_safe(pos, next, &hash_node->notifier_head) {
		tn = qb_list_entry(pos, struct qb_map_notifier, list);
		qb_list_del(&tn->list);
		free(tn);
	}

	qb_list_del(&hash_node->list);
	free(hash_node);
}

static void
hashtable_node_deref(struct qb_map *map, struct hash_node *hash_node)
{
	struct hash_table *t = (struct hash_table *)map;

	hash_node->refcount--;
	if (hash_node->refcount > 0) {
		return;
	}
	hashtable_node_destroy(t, hash_node);
}

static int32_t
hashtable_rm_with_hash(struct qb_map *map, const char *key, uint32_t hash_entry)
{
	struct hash_table *hash_table = (struct hash_table *)map;
	struct qb_list_head *list;
	struct qb_list_head *next;
	struct hash_node *hash_node;

	if (pthread_mutex_lock(&hash_table->hash_buckets[hash_entry].bkt_lock)) {
		return QB_FALSE;
	}

	qb_list_for_each_safe(list, next,
	                      &hash_table->hash_buckets[hash_entry].list_head) {

		hash_node = qb_list_entry(list, struct hash_node, list);
		if (strcmp(hash_node->key, key) == 0) {

			hashtable_node_deref(map, hash_node);
			pthread_mutex_unlock(&hash_table->hash_buckets[hash_entry].bkt_lock);
			(void)pthread_mutex_lock(&hash_table->ht_lock);
			hash_table->count--;
			pthread_mutex_unlock(&hash_table->ht_lock);

			return QB_TRUE;
		}
	}
	pthread_mutex_unlock(&hash_table->hash_buckets[hash_entry].bkt_lock);

	return QB_FALSE;
}

static int32_t
hashtable_rm(struct qb_map *map, const char *key)
{
	struct hash_table *hash_table = (struct hash_table *)map;
	uint32_t hash_entry;

	hash_entry = qb_hash_string(key, hash_table->order);
	return hashtable_rm_with_hash(map, key, hash_entry);
}

static void
hashtable_put(struct qb_map *map, const char *key, const void *value)
{
	struct hash_table *hash_table = (struct hash_table *)map;
	uint32_t hash_entry;
	struct hash_node *hash_node = NULL;
	struct hash_node *node_try;
	struct qb_list_head *list;
	struct qb_list_head *nl;

	hash_entry = qb_hash_string(key, hash_table->order);

	if (pthread_mutex_lock(&hash_table->hash_buckets[hash_entry].bkt_lock)) {
		return;
	}
	qb_list_for_each(list, &hash_table->hash_buckets[hash_entry].list_head) {

		node_try = qb_list_entry(list, struct hash_node, list);
		if (strcmp(node_try->key, key) == 0) {
			hash_node = node_try;
			break;
		}
	}

	if (hash_node == NULL) {
		/* For use outside of the mutex */
		const char *local_key;
		void *local_value;

		hash_node = calloc(1, sizeof(struct hash_node));
		if (hash_node == NULL) {
			errno = ENOMEM;
			pthread_mutex_unlock(&hash_table->hash_buckets[hash_entry].bkt_lock);
			return;
		}

		(void)pthread_mutex_lock(&hash_table->ht_lock);
		hash_table->count++;
		pthread_mutex_unlock(&hash_table->ht_lock);

		hash_node->refcount = 1;
		hash_node->key = key;
		hash_node->value = (void *)value;
		qb_list_init(&hash_node->list);

		qb_list_add_tail(&hash_node->list,
				 &hash_table->hash_buckets[hash_entry].
				 list_head);
		qb_list_init(&hash_node->notifier_head);
		local_key = hash_node->key;
		local_value = hash_node->value;
		nl = copy_notify_list(hash_table, hash_node, QB_MAP_NOTIFY_INSERTED);
		pthread_mutex_unlock(&hash_table->hash_buckets[hash_entry].bkt_lock);

		hashtable_notify(nl,
				 local_key, NULL, local_value);

	} else {
		char *old_k = (char *)hash_node->key;
		char *old_v = (void *)hash_node->value;

		hash_node->key = key;
		hash_node->value = (void *)value;
		nl = copy_notify_list(hash_table, hash_node, QB_MAP_NOTIFY_REPLACED);
		pthread_mutex_unlock(&hash_table->hash_buckets[hash_entry].bkt_lock);

		hashtable_notify(nl, old_k, old_v, hash_node->value);
	}
}


static void add_notify_event(struct qb_list_head *head, uint32_t event, struct qb_map_notifier *tn)
{
	struct qb_map_notifier *nn;

	nn = malloc(sizeof(struct qb_map_notifier));
	if (!nn) {
		return;
	}
	memcpy(nn, tn, sizeof(struct qb_map_notifier));
	nn->events = event;
	qb_list_add_tail(&nn->list, head);
}

static struct qb_list_head *
copy_notify_list(struct hash_table *t, struct hash_node *n,
		 uint32_t event)
{
	struct qb_list_head *list;
	struct qb_list_head *new_head;
	struct qb_map_notifier *tn;

	new_head = malloc(sizeof(struct qb_list_head));
	if (!new_head) {
		return NULL;
	}
	qb_list_init(new_head);

	qb_list_for_each(list, &n->notifier_head) {
		tn = qb_list_entry(list, struct qb_map_notifier, list);

		if (tn->events & event) {
			add_notify_event(new_head, event, tn);
		}
	}
	qb_list_for_each(list, &t->notifier_head) {
		tn = qb_list_entry(list, struct qb_map_notifier, list);

		if (tn->events & event) {
			add_notify_event(new_head, event, tn);
		}
		if (((event & QB_MAP_NOTIFY_DELETED) ||
		     (event & QB_MAP_NOTIFY_REPLACED)) &&
		    (tn->events & QB_MAP_NOTIFY_FREE)) {
			add_notify_event(new_head, QB_MAP_NOTIFY_FREE, tn);
		}
	}
	return new_head;
}

static void hashtable_notify(struct qb_list_head *head, const char *key,
				void *old_value, void *value)
{
	struct qb_list_head *list;
	struct qb_list_head *tmp;
	struct qb_map_notifier *tn;

	if (!head) {
		return;
	}

	qb_list_for_each_safe(list, tmp, head) {
		tn = qb_list_entry(list, struct qb_map_notifier, list);
		tn->callback(tn->events, (char *)key,
			     old_value, value, tn->user_data);
		free(tn);
	}
	free(head);
}

static int32_t
hashtable_notify_add(qb_map_t * m, const char *key,
		     qb_map_notify_fn fn, int32_t events, void *user_data)
{
	struct hash_table *t = (struct hash_table *)m;
	struct qb_map_notifier *f;
	struct hash_node *n;
	struct qb_list_head *head = NULL;
	struct qb_list_head *list;
	int add_to_tail = QB_FALSE;

	if (key) {
		n = hashtable_lookup(t, key);
		if (n) {
			head = &n->notifier_head;
		}
	} else {
		head = &t->notifier_head;
	}
	if (head == NULL) {
		return -ENOENT;
	}
	if (events & QB_MAP_NOTIFY_FREE) {
		add_to_tail = QB_TRUE;
	}

	qb_list_for_each(list, head) {
		f = qb_list_entry(list, struct qb_map_notifier, list);

		if (events & QB_MAP_NOTIFY_FREE &&
		    f->events == events) {
			/* only one free notifier */
			return -EEXIST;
		}
		if (f->events == events &&
		    f->user_data == user_data &&
		    f->callback == fn) {
			return -EEXIST;
		}
	}

	f = malloc(sizeof(struct qb_map_notifier));
	if (f == NULL) {
		return -errno;
	}
	f->events = events;
	f->user_data = user_data;
	f->callback = fn;
	qb_list_init(&f->list);

	if (add_to_tail) {
		qb_list_add_tail(&f->list, head);
	} else {
		qb_list_add(&f->list, head);
	}
	return 0;
}

static int32_t
hashtable_notify_del(qb_map_t * m, const char *key,
		     qb_map_notify_fn fn, int32_t events,
		     int32_t cmp_userdata, void *user_data)
{
	struct hash_table *t = (struct hash_table *)m;
	struct qb_map_notifier *f;
	struct hash_node *n;
	struct qb_list_head *head = NULL;
	struct qb_list_head *list;
	struct qb_list_head *next;
	int32_t found = QB_FALSE;

	if (key) {
		n = hashtable_lookup(t, key);
		if (n) {
			head = &n->notifier_head;
		}
	} else {
		head = &t->notifier_head;
	}
	if (head == NULL) {
		return -ENOENT;
	}

	qb_list_for_each_safe(list, next, head) {
		f = qb_list_entry(list, struct qb_map_notifier, list);

		if (f->events == events && f->callback == fn) {
			if (cmp_userdata && (f->user_data == user_data)) {
				found = QB_TRUE;
				qb_list_del(&f->list);
				free(f);
			} else if (!cmp_userdata) {
				found = QB_TRUE;
				qb_list_del(&f->list);
				free(f);
			}
		}
	}
	if (found) {
		return 0;
	} else {
		return -ENOENT;
	}
}

static size_t
hashtable_count_get(struct qb_map *map)
{
	struct hash_table *hash_table = (struct hash_table *)map;
	size_t count;

	(void)pthread_mutex_lock(&hash_table->ht_lock);
	count = hash_table->count;
	pthread_mutex_unlock(&hash_table->ht_lock);
	return count;
}

static qb_map_iter_t *
hashtable_iter_create(struct qb_map *map, const char *prefix)
{
	struct hashtable_iter *i = malloc(sizeof(struct hashtable_iter));
	if (i == NULL) {
		return NULL;
	}
	i->i.m = map;
	i->node = NULL;
	i->bucket = 0;
	return (qb_map_iter_t *) i;
}

static const char *
hashtable_iter_next(qb_map_iter_t * it, void **value)
{
	struct hashtable_iter *hi = (struct hashtable_iter *)it;
	struct hash_table *hash_table = (struct hash_table *)hi->i.m;
	struct qb_list_head *ln;
	struct hash_node *hash_node = NULL;
	int found = QB_FALSE;
	int cont = QB_TRUE;
	int b;
	const char *ret_key;

	if (hi->node == NULL) {
		cont = QB_FALSE;
	}
	for (b = hi->bucket; b < hash_table->hash_buckets_len && !found; b++) {
		if (cont) {
			ln = &hi->node->list;
			cont = QB_FALSE;
		} else {
			ln = &hash_table->hash_buckets[b].list_head;
		}
		hash_node = qb_list_first_entry(ln, struct hash_node, list);

		if (pthread_mutex_lock(&hash_table->hash_buckets[b].bkt_lock)) {
			return NULL;
		}
		qb_list_for_each_entry_from(hash_node,
		                &hash_table->hash_buckets[b].list_head, list) {
			if (hash_node->refcount > 0) {
				found = QB_TRUE;
				hash_node->refcount++;
				hi->bucket = b;
				*value = hash_node->value;
				ret_key = hash_node->key;
				break;
			}
		}
		pthread_mutex_unlock(&hash_table->hash_buckets[b].bkt_lock);
	}

	if (hi->node) {
		pthread_mutex_lock(&hash_table->hash_buckets[hi->bucket].bkt_lock);
		hashtable_node_deref(hi->i.m, hi->node);
		pthread_mutex_unlock(&hash_table->hash_buckets[hi->bucket].bkt_lock);
	}
	if (!found) {
		return NULL;
	}
	hi->node = hash_node;
	return ret_key;
}

static void
hashtable_iter_free(qb_map_iter_t * i)
{
	free(i);
}

static void
hashtable_destroy(struct qb_map *map)
{
	struct hash_table *hash_table = (struct hash_table *)map;
	struct qb_list_head *pos;
	struct qb_list_head *next;
	struct qb_map_notifier *tn;
	int32_t i;

	for (i = 0; i < hash_table->hash_buckets_len; i++) {
		hashtable_node_deref_under_bucket(map, i);
	}

	qb_list_for_each_safe(pos, next, &hash_table->notifier_head) {
		tn = qb_list_entry(pos, struct qb_map_notifier, list);
		qb_list_del(&tn->list);
		free(tn);
	}

	free(hash_table);
}

static void
hashtable_node_deref_under_bucket(struct qb_map *map, int32_t hash_entry)
{
	struct hash_table *hash_table = (struct hash_table *)map;
	struct hash_node *hash_node;
	struct qb_list_head *pos;
	struct qb_list_head *next;

	qb_list_for_each_safe(pos, next,
			      &hash_table->hash_buckets[hash_entry].list_head) {
		hash_node = qb_list_entry(pos, struct hash_node, list);
		hashtable_node_deref(map, hash_node);
		(void)pthread_mutex_lock(&hash_table->ht_lock);
		hash_table->count--;
		pthread_mutex_unlock(&hash_table->ht_lock);
	}
}

qb_map_t *
qb_hashtable_create(size_t max_size)
{
	int32_t i;
	int32_t order;
	int32_t n = max_size;
	uint64_t size;
	struct hash_table *ht;

	for (i = 0; n; i++) {
		n >>= 1;
	}
	order = QB_MAX(i, 3);

	size = sizeof(struct hash_table) +
	    (sizeof(struct hash_bucket) * (1 << order));

	ht = calloc(1, size);
	if (ht == NULL) {
		return NULL;
	}

	ht->map.put = hashtable_put;
	ht->map.get = hashtable_get;
	ht->map.rm = hashtable_rm;
	ht->map.count_get = hashtable_count_get;
	ht->map.iter_create = hashtable_iter_create;
	ht->map.iter_next = hashtable_iter_next;
	ht->map.iter_free = hashtable_iter_free;
	ht->map.destroy = hashtable_destroy;
	ht->map.notify_add = hashtable_notify_add;
	ht->map.notify_del = hashtable_notify_del;
	ht->count = 0;
	ht->order = order;
	pthread_mutex_init(&ht->ht_lock, NULL);
	qb_list_init(&ht->notifier_head);

	ht->hash_buckets_len = 1 << order;
	for (i = 0; i < ht->hash_buckets_len; i++) {
		qb_list_init(&ht->hash_buckets[i].list_head);
		pthread_mutex_init(&ht->hash_buckets[i].bkt_lock, NULL);
	}
	return (qb_map_t *) ht;
}
