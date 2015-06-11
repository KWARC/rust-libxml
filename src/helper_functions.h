/*
 * This is a small extension to the libxml API.
 * The reason is that we don't convert the structs properly,
 * which prevents some very basic actions
 */
#include <libxml/tree.h>

/*
 * helper functions for tree
 */

//returns cur->next
xmlNodePtr xmlNextSibling(const xmlNodePtr cur);

//returns cur->prev
xmlNodePtr xmlPrevSibling(const xmlNodePtr cur);

//returns cur->children
xmlNodePtr xmlGetFirstChild(const xmlNodePtr cur);

//returns cur->name
const char * xmlNodeGetName(const xmlNodePtr cur);


//returns 1 if text node, 0 otherwise
int xmlIsTextNode(const xmlNodePtr cur);

//returns cur->content
//(Different from xmlNodeGetContent)
const char * xmlNodeGetContentPointer(const xmlNodePtr cur);
